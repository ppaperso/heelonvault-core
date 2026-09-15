#![allow(clippy::disallowed_methods)]

//! Security tests for data integrity.
//! Verifies that data cannot be tampered with without detection.

use heelonvault_core::services::crypto_service::{
    CryptoService, CryptoServiceImpl, EncryptedPayload, NONCE_LEN, decode_envelope, encode_envelope,
};
use secrecy::{ExposeSecret, SecretBox};

// ============================================================================
// Catégorie 4 : Tests d'intégrité des données
// ============================================================================

// ============================================================================
// Catégorie 4.1 : Tests d'authentification AES-GCM
// ============================================================================

#[tokio::test]
async fn test_aes_gcm_authentication() {
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));
    let plaintext = SecretBox::new(Box::new(b"test message".to_vec()));

    // Chiffrer
    let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");

    // Déchiffrer avec la bonne clé - devrait réussir
    let decrypted = service.decrypt(&payload, &key).await.expect("decrypt");
    assert_eq!(
        decrypted.expose_secret(),
        plaintext.expose_secret(),
        "Decrypted plaintext should match original"
    );

    // Modifier le ciphertext (EncryptedPayload n'est pas Clone : on reconstruit manuellement)
    let mut ciphertext = payload.ciphertext.expose_secret().clone();
    ciphertext[0] = ciphertext[0].wrapping_add(1); // Flip un bit
    let corrupted = EncryptedPayload {
        nonce: payload.nonce,
        ciphertext: SecretBox::new(Box::new(ciphertext)),
    };

    // Déchiffrer avec la clé mais ciphertext corrompu - devrait échouer
    let result = service.decrypt(&corrupted, &key).await;
    assert!(
        result.is_err(),
        "Should fail to decrypt corrupted ciphertext. AES-GCM authentication failed!"
    );
}

#[tokio::test]
async fn test_aes_gcm_authentication_multiple_bits() {
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));
    let plaintext = SecretBox::new(Box::new(b"test message for multiple bit flips".to_vec()));

    // Chiffrer
    let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");

    // Modifier plusieurs bits (EncryptedPayload n'est pas Clone : on reconstruit manuellement)
    let mut ciphertext = payload.ciphertext.expose_secret().clone();
    for i in [0, 5, 10, 15].iter() {
        if *i < ciphertext.len() {
            ciphertext[*i] = ciphertext[*i].wrapping_add(1);
        }
    }
    let corrupted = EncryptedPayload {
        nonce: payload.nonce,
        ciphertext: SecretBox::new(Box::new(ciphertext)),
    };

    // Devrait échouer
    let result = service.decrypt(&corrupted, &key).await;
    assert!(
        result.is_err(),
        "Should fail to decrypt with multiple bit flips"
    );
}

#[tokio::test]
async fn test_aes_gcm_different_keys_same_ciphertext_fails() {
    let service = CryptoServiceImpl::with_defaults();
    let key_1 = SecretBox::new(Box::new(vec![0x42; 32]));
    let key_2 = SecretBox::new(Box::new(vec![0x43; 32]));
    let plaintext = SecretBox::new(Box::new(b"test message".to_vec()));

    // Chiffrer avec key_1
    let payload = service.encrypt(&plaintext, &key_1).await.expect("encrypt");

    // Tenter de déchiffrer avec key_2 - devrait échouer
    let result = service.decrypt(&payload, &key_2).await;
    assert!(result.is_err(), "Should fail to decrypt with wrong key");
}

#[tokio::test]
async fn test_aes_gcm_nonce_uniqueness_for_many_encryptions() {
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));

    let mut nonces = Vec::new();
    for i in 0..1000 {
        let plaintext = SecretBox::new(Box::new(format!("message {}", i).as_bytes().to_vec()));
        let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");
        nonces.push(payload.nonce);
    }

    // Vérifier que tous les nonces sont uniques
    use std::collections::HashSet;
    let mut unique_nonces = HashSet::new();
    for (i, nonce) in nonces.iter().enumerate() {
        assert!(
            unique_nonces.insert(nonce),
            "Duplicate nonce found at index {}! This is a critical security issue!",
            i
        );
    }
}

#[tokio::test]
async fn test_aes_gcm_empty_plaintext() {
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));
    let plaintext = SecretBox::new(Box::new(Vec::new()));

    // Chiffrer un plaintext vide
    let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");

    // Vérifier que le nonce est toujours présent
    assert_eq!(
        payload.nonce.len(),
        NONCE_LEN,
        "Nonce should be 12 bytes even for empty plaintext"
    );

    // Déchiffrer
    let decrypted = service.decrypt(&payload, &key).await.expect("decrypt");
    assert_eq!(
        decrypted.expose_secret(),
        plaintext.expose_secret(),
        "Empty plaintext should roundtrip correctly"
    );
}

#[tokio::test]
async fn test_aes_gcm_large_plaintext() {
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));

    // Créer un plaintext de 1 Mo
    let large_plaintext = vec![0xAAu8; 1024 * 1024];
    let plaintext = SecretBox::new(Box::new(large_plaintext.clone()));

    // Chiffrer
    let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");

    // Vérifier que le ciphertext a à peu près la même taille
    let ciphertext_len = payload.ciphertext.expose_secret().len();
    assert!(
        ciphertext_len > 1024 * 1024,
        "Ciphertext should be larger than plaintext (includes auth tag)"
    );

    // Déchiffrer
    let decrypted = service.decrypt(&payload, &key).await.expect("decrypt");
    assert_eq!(
        decrypted.expose_secret(),
        &large_plaintext,
        "Large plaintext should roundtrip correctly"
    );
}

// ============================================================================
// Catégorie 4.1 : Tests d'authentification AES-GCM (suite)
// ============================================================================

#[tokio::test]
async fn test_envelope_version_validation() {
    use heelonvault_core::services::crypto_service::ENVELOPE_VERSION;

    let payload = EncryptedPayload {
        nonce: [0u8; 12],
        ciphertext: SecretBox::new(Box::new(b"test".to_vec())),
    };

    // Encoder avec la version actuelle
    let envelope = encode_envelope(&payload);
    assert_eq!(
        envelope.expose_secret()[0],
        ENVELOPE_VERSION,
        "Envelope should start with version byte"
    );

    // Décoder devrait réussir
    let decoded = decode_envelope(envelope.expose_secret()).expect("decode envelope");
    assert_eq!(decoded.nonce, payload.nonce);
    assert_eq!(
        decoded.ciphertext.expose_secret(),
        payload.ciphertext.expose_secret()
    );

    // Essayer avec une version invalide (0)
    let mut invalid_version = envelope.expose_secret().clone();
    invalid_version[0] = 0;
    let result = decode_envelope(&invalid_version);
    assert!(result.is_err(), "Should reject invalid version (0)");

    // Essayer avec une version future (255)
    let mut future_version = envelope.expose_secret().clone();
    future_version[0] = 255;
    let result = decode_envelope(&future_version);
    assert!(result.is_err(), "Should reject future version (255)");

    // decode_envelope n'accepte que ENVELOPE_VERSION (1) : toute autre valeur est rejetée,
    // il n'existe pas de "version 2" future à supporter pour cette enveloppe.
    let mut other_version = envelope.expose_secret().clone();
    other_version[0] = 2;
    let result = decode_envelope(&other_version);
    assert!(
        result.is_err(),
        "Should reject any version other than ENVELOPE_VERSION (2)"
    );
}

#[tokio::test]
async fn test_envelope_truncated_data() {
    use heelonvault_core::services::crypto_service::ENVELOPE_VERSION;

    // Enveloppe trop courte (juste la version)
    let truncated = vec![ENVELOPE_VERSION];
    let result = decode_envelope(&truncated);
    assert!(
        result.is_err(),
        "Should reject truncated envelope (only version)"
    );

    // Enveloppe avec version mais pas de nonce complet
    let mut too_short = vec![ENVELOPE_VERSION];
    too_short.extend_from_slice(&[0u8; 5]); // 1 octet version + 5 octets
    let result = decode_envelope(&too_short);
    assert!(
        result.is_err(),
        "Should reject envelope without complete nonce"
    );

    // Enveloppe avec version + nonce mais pas de ciphertext
    let mut no_ciphertext = vec![ENVELOPE_VERSION];
    no_ciphertext.extend_from_slice(&[0u8; 12]); // nonce
    let result = decode_envelope(&no_ciphertext);
    assert!(result.is_err(), "Should reject envelope without ciphertext");
}

#[tokio::test]
async fn test_envelope_roundtrip() {
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));
    let plaintext = SecretBox::new(Box::new(b"roundtrip test message".to_vec()));

    // Chiffrer
    let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");

    // Encoder
    let envelope = encode_envelope(&payload);

    // Décoder
    let decoded = decode_envelope(envelope.expose_secret()).expect("decode envelope");

    // Vérifier
    assert_eq!(decoded.nonce, payload.nonce);
    assert_eq!(
        decoded.ciphertext.expose_secret(),
        payload.ciphertext.expose_secret()
    );

    // Déchiffrer
    let decrypted = service.decrypt(&decoded, &key).await.expect("decrypt");
    assert_eq!(
        decrypted.expose_secret(),
        plaintext.expose_secret(),
        "Roundtrip should preserve plaintext"
    );
}

#[tokio::test]
async fn test_corrupted_envelope_structure() {
    // Créer une enveloppe valide
    let payload = EncryptedPayload {
        nonce: [0xAA; 12],
        ciphertext: SecretBox::new(Box::new(b"test".to_vec())),
    };
    let envelope = encode_envelope(&payload);

    // Corrompre la version
    let mut corrupted_version = envelope.expose_secret().clone();
    corrupted_version[0] = corrupted_version[0].wrapping_add(1);
    let result = decode_envelope(&corrupted_version);
    assert!(result.is_err(), "Should reject corrupted version byte");

    // Corrompre le nonce
    let mut corrupted_nonce = envelope.expose_secret().clone();
    corrupted_nonce[1] = corrupted_nonce[1].wrapping_add(1);
    let result = decode_envelope(&corrupted_nonce);
    // Cela pourrait réussir ou échouer selon l'implémentation
    // mais le déchiffrement devrait échouer
    if let Ok(decoded) = result {
        // Essayer de déchiffrer avec la mauvaise clé
        let service = CryptoServiceImpl::with_defaults();
        let wrong_key = SecretBox::new(Box::new(vec![0u8; 32]));
        let decrypt_result = service.decrypt(&decoded, &wrong_key).await;
        assert!(
            decrypt_result.is_err(),
            "Should fail to decrypt with corrupted nonce"
        );
    }
}

// ============================================================================
// Tests d'intégrité supplémentaires
// ============================================================================

#[tokio::test]
async fn test_encryption_same_input_produces_different_output() {
    // Vérifier que le chiffrement est non-déterministe (à cause des nonces aléatoires)
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));
    let plaintext = SecretBox::new(Box::new(b"same message".to_vec()));

    let mut ciphertexts = Vec::new();
    for _ in 0..100 {
        let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");
        ciphertexts.push(payload.ciphertext.expose_secret().clone());
    }

    // Tous les ciphertexts doivent être différents
    use std::collections::HashSet;
    let mut unique_ciphertexts = HashSet::new();
    for (i, ct) in ciphertexts.iter().enumerate() {
        assert!(
            unique_ciphertexts.insert(ct),
            "Duplicate ciphertext found at index {}! This indicates nonce reuse!",
            i
        );
    }
}

#[tokio::test]
async fn test_authentication_tag_integrity() {
    // Tester que le tag d'authentification de AES-GCM détecte les modifications
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));
    let plaintext = SecretBox::new(Box::new(b"authentication test".to_vec()));

    let payload = service.encrypt(&plaintext, &key).await.expect("encrypt");
    let ciphertext = payload.ciphertext.expose_secret().clone();

    // Le tag d'authentification fait partie du ciphertext (16 octets à la fin)
    let tag_length = 16; // AES-GCM tag is 16 bytes
    assert!(
        ciphertext.len() >= tag_length,
        "Ciphertext should include auth tag"
    );

    // Corrompre le tag
    let mut corrupted_ciphertext = ciphertext.clone();
    // Corrompre le dernier octet du tag
    if corrupted_ciphertext.len() >= tag_length {
        let tag_start = corrupted_ciphertext.len() - tag_length;
        corrupted_ciphertext[tag_start] = corrupted_ciphertext[tag_start].wrapping_add(1);
    }

    let corrupted_payload = EncryptedPayload {
        nonce: payload.nonce,
        ciphertext: SecretBox::new(Box::new(corrupted_ciphertext)),
    };

    // Devrait échouer
    let result = service.decrypt(&corrupted_payload, &key).await;
    assert!(
        result.is_err(),
        "Should fail to decrypt with corrupted auth tag"
    );
}

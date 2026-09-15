#![allow(clippy::disallowed_methods)]

//! Security tests for input validation.
//! Verifies that malformed inputs are rejected before any processing.

use heelonvault_core::errors::AppError;
use heelonvault_core::services::crypto_service::{CryptoService, KdfConfig};
use secrecy::{ExposeSecret, SecretBox, SecretString};

// ============================================================================
// Catégorie 8 : Tests de validation des entrées
// ============================================================================

// ============================================================================
// Catégorie 8.1 : Tests de validation
// ============================================================================
//
// Les tests de troncature/version de l'enveloppe crypto générique (`decode_envelope`)
// vivent dans `data_integrity.rs` (ils partent d'une enveloppe réellement produite par
// `encode_envelope` puis la corrompent). Ce fichier se concentre sur des entrées
// adverses/malformées au sens large : formats d'enveloppe de mot de passe, tailles
// limites, Unicode, chaînes de type injection SQL.

#[test]
fn test_password_envelope_decode_rejects_invalid_format() {
    use heelonvault_core::services::auth_service::decode_password_envelope;
    use secrecy::SecretBox;

    // Enveloppe trop courte pour l'header
    let too_short = SecretBox::new(Box::new(vec![1u8; 4]));
    let result = decode_password_envelope(&too_short);
    assert!(result.is_err(), "Should reject too short envelope");

    // Header invalide (salt_len = 0)
    let mut invalid_header = vec![1u8; 5]; // version + 0 + 0 + 0 + 0
    invalid_header[1..=2].copy_from_slice(&0u16.to_be_bytes()); // salt_len = 0
    let envelope = SecretBox::new(Box::new(invalid_header));
    let result = decode_password_envelope(&envelope);
    assert!(
        result.is_err(),
        "Should reject envelope with zero salt length"
    );

    // body_len = 0
    let mut invalid_body_len = vec![1u8; 5];
    invalid_body_len[3..=4].copy_from_slice(&0u16.to_be_bytes()); // body_len = 0
    let envelope = SecretBox::new(Box::new(invalid_body_len));
    let result = decode_password_envelope(&envelope);
    assert!(
        result.is_err(),
        "Should reject envelope with zero body length"
    );

    // Longueur totale incorrecte
    let mut wrong_total_len = vec![1u8; 10];
    // Header dit salt_len=1, body_len=1, mais la longueur totale est 10
    wrong_total_len[1..=2].copy_from_slice(&1u16.to_be_bytes()); // salt_len = 1
    wrong_total_len[3..=4].copy_from_slice(&1u16.to_be_bytes()); // body_len = 1
    // total_len devrait être 5 + 1 + 1 = 7, mais on a 10
    let envelope = SecretBox::new(Box::new(wrong_total_len));
    let result = decode_password_envelope(&envelope);
    assert!(
        result.is_err(),
        "Should reject envelope with incorrect total length"
    );
}

#[tokio::test]
async fn test_password_envelope_version_validation() {
    use heelonvault_core::services::auth_service::{
        PasswordEnvelope, decode_password_envelope, encode_password_envelope,
    };
    use heelonvault_core::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretBox;

    let crypto = CryptoServiceImpl::new(KdfConfig {
        memory_cost_kib: 1024,
        time_cost: 1,
        parallelism: 1,
        output_len: 32,
    });

    // Créer un envelope v2 valide
    let password = SecretString::new("test_password".to_string().into());
    let account_key = SecretBox::new(Box::new(vec![0x42; 32]));
    let envelope = heelonvault_core::services::auth_service::wrap_account_key(
        &crypto,
        &password,
        &account_key,
    )
    .await
    .expect("Failed to wrap");

    let encoded = encode_password_envelope(&envelope);
    let decoded = decode_password_envelope(&encoded).expect("Should decode v2");
    assert!(
        matches!(decoded, PasswordEnvelope::AccountKey { .. }),
        "Should be v2"
    );

    // Corrompre la version pour faire un v0 invalide
    let mut corrupted = encoded.expose_secret().clone();
    corrupted[0] = 0;
    let result = decode_password_envelope(&SecretBox::new(Box::new(corrupted)));
    assert!(result.is_err(), "Should reject v0");

    // Corrompre la version pour faire un v3 invalide
    let mut corrupted_v3 = encoded.expose_secret().clone();
    corrupted_v3[0] = 3;
    let result = decode_password_envelope(&SecretBox::new(Box::new(corrupted_v3)));
    assert!(result.is_err(), "Should reject v3");
}

#[tokio::test]
async fn test_crypto_operations_reject_invalid_inputs() {
    use heelonvault_core::services::crypto_service::{CryptoServiceImpl, EncryptedPayload};
    use secrecy::{SecretBox, SecretString};

    let service = CryptoServiceImpl::with_defaults();

    // Dériver avec un salt vide
    let empty_salt = SecretBox::new(Box::new(Vec::new()));
    let password = SecretString::new("test".to_string().into());
    let result = service.derive_key(&password, &empty_salt).await;
    assert!(result.is_err(), "Should reject empty salt");

    // Déchiffrer avec une clé invalide (trop courte)
    let payload = EncryptedPayload {
        nonce: [0u8; 12],
        ciphertext: SecretBox::new(Box::new(b"test".to_vec())),
    };
    let short_key = SecretBox::new(Box::new(vec![0u8; 16])); // 16 octets au lieu de 32
    let result = service.decrypt(&payload, &short_key).await;
    assert!(
        result.is_err(),
        "Should reject invalid key length (16 bytes)"
    );

    // Déchiffrer avec une clé vide
    let empty_key = SecretBox::new(Box::new(Vec::new()));
    let result = service.decrypt(&payload, &empty_key).await;
    assert!(result.is_err(), "Should reject empty key");
}

#[tokio::test]
async fn test_encryption_rejects_empty_plaintext_with_validation() {
    // Note: L'implémentation actuelle peut accepter un plaintext vide
    // mais ce test vérifie que cela ne cause pas de panique
    use heelonvault_core::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretBox;

    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));
    let empty_plaintext = SecretBox::new(Box::new(Vec::new()));

    // Cela devrait fonctionner (chiffrement d'un plaintext vide)
    let result = service.encrypt(&empty_plaintext, &key).await;
    assert!(result.is_ok(), "Empty plaintext should be encryptable");
}

#[tokio::test]
async fn test_recovery_phrase_format_validation() {
    use heelonvault_core::services::account_key::{
        open_with_recovery_phrase, seal_with_recovery_phrase,
    };
    use heelonvault_core::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretString;

    let crypto = CryptoServiceImpl::new(KdfConfig {
        memory_cost_kib: 1024,
        time_cost: 1,
        parallelism: 1,
        output_len: 32,
    });

    // Sceller avec une phrase valide
    let valid_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident account";
    let account_key = SecretBox::new(Box::new(vec![0x42; 32]));
    let envelope = seal_with_recovery_phrase(
        &crypto,
        &SecretString::new(valid_phrase.to_string().into()),
        &account_key,
    )
    .await
    .expect("Should seal with valid phrase");

    // Ouvrir avec la même phrase devrait fonctionner
    let result = open_with_recovery_phrase(
        &crypto,
        &SecretString::new(valid_phrase.to_string().into()),
        &envelope,
    )
    .await;
    assert!(result.is_ok(), "Should open with valid phrase");

    // Ouvrir avec une phrase invalide devrait échouer
    let result = open_with_recovery_phrase(
        &crypto,
        &SecretString::new("invalid phrase".to_string().into()),
        &envelope,
    )
    .await;
    assert!(result.is_err(), "Should fail with invalid phrase");
}

#[test]
fn test_envelope_header_validation() {
    use heelonvault_core::services::auth_service::decode_password_envelope;
    use secrecy::SecretBox;

    // Header trop court (moins de 5 octets)
    let too_short = SecretBox::new(Box::new(vec![1u8, 0, 0])); // 3 octets
    let result = decode_password_envelope(&too_short);
    assert!(result.is_err(), "Should reject header too short");

    // Version invalide dans l'header
    let invalid_version = SecretBox::new(Box::new(vec![0u8, 0, 1, 0, 1])); // version=0
    let result = decode_password_envelope(&invalid_version);
    assert!(result.is_err(), "Should reject invalid version in header");

    // Version 1 valide mais structure invalide
    let version_1 = SecretBox::new(Box::new(vec![1u8, 0, 0, 0, 0])); // version=1, salt_len=0, body_len=0
    let result = decode_password_envelope(&version_1);
    assert!(result.is_err(), "Should reject version 1 with zero lengths");
}

#[test]
fn test_payload_structure_validation() {
    use heelonvault_core::services::crypto_service::decode_envelope;

    // Enveloppe avec une version valide mais pas de nonce
    let invalid_nonce = vec![1u8; 2]; // version + 1 octet
    let result = decode_envelope(&invalid_nonce);
    assert!(
        result.is_err(),
        "Should reject envelope with incomplete nonce"
    );

    // Enveloppe avec version + nonce mais pas de ciphertext
    let no_ciphertext = vec![1u8; 13]; // version + 12 octets nonce
    let result = decode_envelope(&no_ciphertext);
    assert!(result.is_err(), "Should reject envelope without ciphertext");
}

// ============================================================================
// Tests supplémentaires de validation
// ============================================================================

#[tokio::test]
async fn test_derive_key_with_empty_password() {
    use heelonvault_core::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretString;

    let service = CryptoServiceImpl::with_defaults();
    let empty_password = SecretString::new("".to_string().into());
    let salt = service
        .generate_kdf_salt()
        .await
        .expect("Salt generation failed");

    // Argon2id devrait accepter un mot de passe vide (même si ce n'est pas recommandé)
    // mais cela ne devrait pas causer de panique
    let result = service.derive_key(&empty_password, &salt).await;
    assert!(
        result.is_ok(),
        "Empty password should be derivable (though not recommended)"
    );

    // La clé devrait quand même avoir la bonne longueur
    let key = result.expect("key derived from empty password");
    assert_eq!(
        key.expose_secret().len(),
        32,
        "Key should be 32 bytes even with empty password"
    );
}

#[tokio::test]
async fn test_derive_key_with_very_long_password() {
    use heelonvault_core::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretString;

    let service = CryptoServiceImpl::with_defaults();

    // Créer un mot de passe très long (1 Mo)
    let long_password = "a".repeat(1024 * 1024);
    let password = SecretString::new(long_password.into());
    let salt = service
        .generate_kdf_salt()
        .await
        .expect("Salt generation failed");

    // Cela devrait fonctionner (Argon2id gère les longs passwords)
    let result = service.derive_key(&password, &salt).await;
    assert!(result.is_ok(), "Long password should be derivable");

    let key = result.expect("key derived from a very long password");
    assert_eq!(key.expose_secret().len(), 32, "Key should be 32 bytes");
}

#[test]
fn test_validation_error_messages() {
    use heelonvault_core::services::crypto_service::decode_envelope;

    // Enveloppe trop courte
    let truncated = vec![1u8];
    let result = decode_envelope(&truncated);
    assert!(result.is_err(), "Should fail");

    if let Err(AppError::Validation(msg)) = result {
        assert!(
            msg.contains("truncated") || msg.contains("short"),
            "Error message should mention truncation, got: {}",
            msg
        );
    }

    // Version invalide (14 octets : version + nonce complet + 1 octet de ciphertext,
    // pour dépasser le seuil de troncature et atteindre réellement la vérification de version)
    let invalid_version = vec![0u8; 14];
    let result = decode_envelope(&invalid_version);
    assert!(result.is_err(), "Should fail");

    if let Err(AppError::Validation(msg)) = result {
        assert!(
            msg.contains("version") || msg.contains("unsupported"),
            "Error message should mention version, got: {}",
            msg
        );
    }
}

#[tokio::test]
async fn test_sql_injection_like_inputs() {
    // Tester que des entrées qui ressemblent à des injections SQL sont rejetées
    // si elles sont utilisées dans des requêtes

    // Ces tests sont plus pertinents pour les tests d'intégration avec la DB,
    // mais on vérifie au moins que les fonctions de chiffrement ne paniquent pas

    use heelonvault_core::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretBox;

    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![0x42; 32]));

    // Essayer de chiffrer une chaîne qui ressemble à une injection SQL
    let sql_like = SecretBox::new(Box::new(b"'; DROP TABLE users; --".to_vec()));
    let result = service.encrypt(&sql_like, &key).await;
    assert!(
        result.is_ok(),
        "Should be able to encrypt SQL-like strings (they're just data)"
    );

    // Le chiffrement devrait produire un ciphertext valide
    let payload = result.expect("encrypt SQL-like plaintext");
    assert!(
        !payload.ciphertext.expose_secret().is_empty(),
        "Ciphertext should not be empty"
    );
}

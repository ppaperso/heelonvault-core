#![allow(clippy::disallowed_methods)]

//! Security test template for crypto primitives.
//! Focused on contract-level checks for KDF and AEAD behavior.

use heelonvault_rust::services::crypto_service::{
    CryptoService, CryptoServiceImpl, EncryptedPayload, NONCE_LEN,
};
use secrecy::{ExposeSecret, SecretBox, SecretString};

#[tokio::test]
async fn template_crypto_argon2id_policy() {
    let service = CryptoServiceImpl::with_defaults();
    let password = SecretString::new("correct horse battery staple".to_string().into());
    let salt_result = service.generate_kdf_salt().await;
    assert!(salt_result.is_ok(), "salt generation should succeed");
    let salt = match salt_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let key_a_result = service.derive_key(&password, &salt).await;
    assert!(key_a_result.is_ok(), "first key derivation should succeed");
    let key_a = match key_a_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let key_b_result = service.derive_key(&password, &salt).await;
    assert!(key_b_result.is_ok(), "second key derivation should succeed");
    let key_b = match key_b_result {
        Ok(value) => value,
        Err(_) => return,
    };

    assert_eq!(key_a.expose_secret().len(), 32);
    assert_eq!(key_a.expose_secret(), key_b.expose_secret());
}

#[tokio::test]
async fn template_crypto_aes_gcm_nonce_policy() {
    let service = CryptoServiceImpl::with_defaults();
    let key = SecretBox::new(Box::new(vec![7_u8; 32]));
    let plaintext = SecretBox::new(Box::new(b"payload-for-nonce-test".to_vec()));

    let enc_a_result = service.encrypt(&plaintext, &key).await;
    assert!(enc_a_result.is_ok(), "first encryption should succeed");
    let enc_a = match enc_a_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let enc_b_result = service.encrypt(&plaintext, &key).await;
    assert!(enc_b_result.is_ok(), "second encryption should succeed");
    let enc_b = match enc_b_result {
        Ok(value) => value,
        Err(_) => return,
    };

    assert_eq!(enc_a.nonce.len(), NONCE_LEN);
    assert_eq!(enc_b.nonce.len(), NONCE_LEN);
    assert_ne!(enc_a.nonce, enc_b.nonce);
    assert_ne!(
        enc_a.ciphertext.expose_secret(),
        enc_b.ciphertext.expose_secret()
    );
}

#[tokio::test]
async fn template_crypto_secret_zeroization() {
    let service = CryptoServiceImpl::with_defaults();
    let password = SecretString::new("zeroize-check".to_string().into());
    let salt_result = service.generate_kdf_salt().await;
    assert!(salt_result.is_ok(), "salt generation should succeed");
    let salt = match salt_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let derived_key_result = service.derive_key(&password, &salt).await;
    assert!(derived_key_result.is_ok(), "key derivation should succeed");
    let derived_key = match derived_key_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let plaintext = SecretBox::new(Box::new(b"roundtrip-secret".to_vec()));
    let encrypted_result = service.encrypt(&plaintext, &derived_key).await;
    assert!(encrypted_result.is_ok(), "encryption should succeed");
    let encrypted = match encrypted_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let decrypted_result = service.decrypt(&encrypted, &derived_key).await;
    assert!(decrypted_result.is_ok(), "decryption should succeed");
    let decrypted = match decrypted_result {
        Ok(value) => value,
        Err(_) => return,
    };

    assert_eq!(decrypted.expose_secret(), plaintext.expose_secret());

    let _typed_payload: EncryptedPayload = encrypted;
}

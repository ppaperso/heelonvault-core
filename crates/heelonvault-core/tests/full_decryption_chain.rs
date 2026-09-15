#![allow(clippy::disallowed_methods)]

//! Security tests for the complete decryption chain.
//! Verifies that the entire chain of decryption works correctly and that
//! omitting any step fails.

use heelonvault_core::services::account_key::{
    generate_account_key, open_with_recovery_phrase, seal_with_recovery_phrase,
};
use heelonvault_core::services::auth_service::{
    PasswordEnvelope, decode_password_envelope, encode_password_envelope, unlock_password_envelope,
    wrap_account_key,
};
use heelonvault_core::services::crypto_service::{
    CryptoService, CryptoServiceImpl, decode_envelope, encode_envelope,
};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use uuid::Uuid;

/// Helper to create a complete test environment with user, vault, and secret.
/// Returns the password/recovery envelopes it built so callers reuse them instead of
/// re-deriving their own copies.
#[allow(clippy::type_complexity)]
async fn setup_complete_environment(
    crypto: &CryptoServiceImpl,
    password: &str,
    recovery_phrase: &str,
) -> (
    Uuid,
    Uuid,
    Uuid,
    SecretBox<Vec<u8>>,
    PasswordEnvelope,
    SecretBox<Vec<u8>>,
    SecretBox<Vec<u8>>,
    SecretBox<Vec<u8>>,
) {
    let user_id = Uuid::new_v4();
    let vault_id = Uuid::new_v4();
    let secret_id = Uuid::new_v4();

    // Generate account key
    let account_key = generate_account_key().expect("Failed to generate account key");

    // Wrap account key with password (v2 envelope)
    let password_envelope = wrap_account_key(
        crypto,
        &SecretString::new(password.to_string().into()),
        &account_key,
    )
    .await
    .expect("Failed to wrap account key");

    // Seal account key with recovery phrase
    let recovery_envelope = seal_with_recovery_phrase(
        crypto,
        &SecretString::new(recovery_phrase.to_string().into()),
        &account_key,
    )
    .await
    .expect("Failed to seal with recovery phrase");

    // Generate vault key and encrypt with account key
    let vault_key = generate_account_key().expect("Failed to generate vault key");
    let encrypted_vault_key = crypto
        .encrypt(&vault_key, &account_key)
        .await
        .expect("Failed to encrypt vault key");
    let vault_key_envelope = encode_envelope(&encrypted_vault_key);

    // Generate secret and encrypt with vault key
    let secret_plaintext = SecretBox::new(Box::new(b"my_super_secret_data_123!".to_vec()));
    let encrypted_secret = crypto
        .encrypt(&secret_plaintext, &vault_key)
        .await
        .expect("Failed to encrypt secret");
    let secret_blob = encode_envelope(&encrypted_secret);

    (
        user_id,
        vault_id,
        secret_id,
        account_key,
        password_envelope,
        recovery_envelope,
        vault_key_envelope,
        secret_blob,
    )
}

// ============================================================================
// Catégorie 5 : Tests de la chaîne de déchiffrement complète
// ============================================================================

#[tokio::test]
async fn test_full_decryption_chain_with_password() {
    let crypto = CryptoServiceImpl::with_defaults();
    let password = "MyStr0ng!Passw0rd";
    let recovery_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident account";

    let (
        _,
        _,
        _,
        _account_key_original,
        password_envelope,
        _recovery_envelope,
        vault_key_envelope,
        secret_blob,
    ) = setup_complete_environment(&crypto, password, recovery_phrase).await;

    // Étape 1: Déverrouiller l'account key avec le mot de passe
    // (simulation: on a le password_envelope en DB)
    let encoded_password_envelope = encode_password_envelope(&password_envelope);
    let decoded_password_envelope = decode_password_envelope(&encoded_password_envelope)
        .expect("Failed to decode password envelope");

    let account_key_result = unlock_password_envelope(
        &crypto,
        &decoded_password_envelope,
        &SecretString::new(password.to_string().into()),
    )
    .await
    .expect("Failed to unlock password envelope");

    let account_key = account_key_result.expect("Password should unlock the account key");

    // Étape 2: Déchiffrer la vault key avec l'account key
    let vault_payload = decode_envelope(vault_key_envelope.expose_secret())
        .expect("Failed to decode vault key envelope");
    let vault_key_result = crypto.decrypt(&vault_payload, &account_key).await;

    assert!(vault_key_result.is_ok(), "Step 2 failed: decrypt vault key");
    let vault_key = vault_key_result.expect("vault key decrypted in step 2");

    // Étape 3: Déchiffrer le secret avec la vault key
    let secret_payload =
        decode_envelope(secret_blob.expose_secret()).expect("Failed to decode secret blob");
    let secret_result = crypto.decrypt(&secret_payload, &vault_key).await;

    assert!(secret_result.is_ok(), "Step 3 failed: decrypt secret");
    let decrypted_secret = secret_result.expect("secret decrypted in step 3");

    // Vérifier que le secret déchiffré correspond au plaintext original
    assert_eq!(
        decrypted_secret.expose_secret(),
        b"my_super_secret_data_123!",
        "Decrypted secret does not match original plaintext"
    );
}

#[tokio::test]
async fn test_full_decryption_chain_requires_all_components() {
    let crypto = CryptoServiceImpl::with_defaults();
    let password = "MyStr0ng!Passw0rd";
    let recovery_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident account";

    let (
        _,
        _,
        _,
        _account_key_original,
        password_envelope,
        _recovery_envelope,
        vault_key_envelope,
        secret_blob,
    ) = setup_complete_environment(&crypto, password, recovery_phrase).await;

    // Récupérer toutes les enveloppes
    let encoded_password_envelope = encode_password_envelope(&password_envelope);
    let decoded_password_envelope = decode_password_envelope(&encoded_password_envelope)
        .expect("Failed to decode password envelope");

    let vault_payload = decode_envelope(vault_key_envelope.expose_secret())
        .expect("Failed to decode vault key envelope");
    let secret_payload =
        decode_envelope(secret_blob.expose_secret()).expect("Failed to decode secret blob");

    // Maintenant, tester que l'omission de n'importe quelle étape échoue

    // Test 1: Sans le bon mot de passe
    let wrong_password_result = unlock_password_envelope(
        &crypto,
        &decoded_password_envelope,
        &SecretString::new("wrong_password".to_string().into()),
    )
    .await
    .expect("Should not panic");

    assert!(
        wrong_password_result.is_none(),
        "Should not unlock with wrong password"
    );

    // Test 2: Avec le bon mot de passe mais en essayant de déchiffrer la vault key avec la mauvaise clé
    let account_key = unlock_password_envelope(
        &crypto,
        &decoded_password_envelope,
        &SecretString::new(password.to_string().into()),
    )
    .await
    .expect("Failed to unlock")
    .expect("Password should work");

    let wrong_vault_key = SecretBox::new(Box::new(vec![0u8; 32]));
    let result = crypto.decrypt(&vault_payload, &wrong_vault_key).await;
    assert!(result.is_err(), "Should fail with wrong vault key");

    // Test 3: Avec la bonne vault key mais en essayant de déchiffrer le secret avec la mauvaise clé
    let vault_key = crypto
        .decrypt(&vault_payload, &account_key)
        .await
        .expect("Should decrypt vault key");
    let wrong_secret_key = SecretBox::new(Box::new(vec![0u8; 32]));
    let result = crypto.decrypt(&secret_payload, &wrong_secret_key).await;
    assert!(result.is_err(), "Should fail with wrong secret key");

    // Test 4: Vérifier que la bonne combinaison fonctionne
    let correct_result = crypto.decrypt(&secret_payload, &vault_key).await;
    assert!(correct_result.is_ok(), "Should succeed with correct key");
}

#[tokio::test]
async fn test_recovery_phrase_alternative_path() {
    // Tester que la recovery phrase permet aussi de déchiffrer l'account key
    let crypto = CryptoServiceImpl::with_defaults();
    let password = "MyStr0ng!Passw0rd";
    let recovery_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident account";

    let (
        _,
        _,
        _,
        account_key_original,
        _password_envelope,
        recovery_envelope,
        vault_key_envelope,
        secret_blob,
    ) = setup_complete_environment(&crypto, password, recovery_phrase).await;

    // Déverrouiller l'account key avec la recovery phrase
    let account_key_result = open_with_recovery_phrase(
        &crypto,
        &SecretString::new(recovery_phrase.to_string().into()),
        &recovery_envelope,
    )
    .await
    .expect("Should unlock with recovery phrase");

    assert_eq!(
        account_key_result.expose_secret(),
        account_key_original.expose_secret(),
        "Account key from recovery should match original"
    );

    // Maintenant, on peut déchiffrer la vault key
    let vault_payload = decode_envelope(vault_key_envelope.expose_secret())
        .expect("Failed to decode vault key envelope");
    let vault_key_result = crypto.decrypt(&vault_payload, &account_key_result).await;

    assert!(
        vault_key_result.is_ok(),
        "Should decrypt vault key with account key from recovery"
    );

    // Et enfin le secret
    let secret_payload =
        decode_envelope(secret_blob.expose_secret()).expect("Failed to decode secret blob");
    let vault_key = vault_key_result.expect("vault key decrypted from recovery path");
    let secret_result = crypto.decrypt(&secret_payload, &vault_key).await;

    assert!(
        secret_result.is_ok(),
        "Should decrypt secret with vault key from recovery"
    );

    // Vérifier que le secret est correct
    assert_eq!(
        secret_result
            .expect("secret decrypted from recovery path")
            .expose_secret(),
        b"my_super_secret_data_123!",
        "Secret decrypted via recovery path should match original"
    );
}

#[tokio::test]
async fn test_wrong_recovery_phrase_fails() {
    let crypto = CryptoServiceImpl::with_defaults();
    let password = "MyStr0ng!Passw0rd";
    let recovery_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident account";
    let wrong_phrase = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    let (_, _, _, _account_key_original, _password_envelope, recovery_envelope, _, _) =
        setup_complete_environment(&crypto, password, recovery_phrase).await;

    // Try to open with wrong phrase
    let result = open_with_recovery_phrase(
        &crypto,
        &SecretString::new(wrong_phrase.to_string().into()),
        &recovery_envelope,
    )
    .await;

    assert!(
        result.is_err(),
        "Should fail to open with wrong recovery phrase"
    );
}

#[tokio::test]
async fn test_account_key_consistency_across_paths() {
    // Vérifier que l'account key obtenue via le mot de passe est la même que celle via la recovery phrase
    let crypto = CryptoServiceImpl::with_defaults();
    let password = "MyStr0ng!Passw0rd";
    let recovery_phrase = "abandon ability able about above absent absorb abstract absurd abuse access accident account";

    let account_key_original = generate_account_key().expect("Failed to generate account key");

    // Wrap with password
    let password_envelope = wrap_account_key(
        &crypto,
        &SecretString::new(password.to_string().into()),
        &account_key_original,
    )
    .await
    .expect("Failed to wrap");

    // Seal with recovery phrase
    let recovery_envelope = seal_with_recovery_phrase(
        &crypto,
        &SecretString::new(recovery_phrase.to_string().into()),
        &account_key_original,
    )
    .await
    .expect("Failed to seal");

    // Unlock via password
    let encoded_password_envelope = encode_password_envelope(&password_envelope);
    let decoded_password_envelope =
        decode_password_envelope(&encoded_password_envelope).expect("Failed to decode");
    let account_key_from_password = unlock_password_envelope(
        &crypto,
        &decoded_password_envelope,
        &SecretString::new(password.to_string().into()),
    )
    .await
    .expect("Failed to unlock")
    .expect("Should unlock");

    // Unlock via recovery phrase
    let account_key_from_recovery = open_with_recovery_phrase(
        &crypto,
        &SecretString::new(recovery_phrase.to_string().into()),
        &recovery_envelope,
    )
    .await
    .expect("Failed to open");

    // Les deux doivent donner la même account key
    assert_eq!(
        account_key_from_password.expose_secret(),
        account_key_from_recovery.expose_secret(),
        "Account key from password and recovery should be identical"
    );

    assert_eq!(
        account_key_from_password.expose_secret(),
        account_key_original.expose_secret(),
        "Account keys should match original"
    );
}

#[tokio::test]
async fn test_envelope_version_validation_in_chain() {
    // Vérifier que les enveloppes avec des versions invalides sont rejetées
    let crypto = CryptoServiceImpl::with_defaults();

    // Créer une enveloppe valide
    let account_key = generate_account_key().expect("Failed to generate account key");
    let password = "test_password";
    let password_envelope = wrap_account_key(
        &crypto,
        &SecretString::new(password.to_string().into()),
        &account_key,
    )
    .await
    .expect("Failed to wrap");

    let encoded = encode_password_envelope(&password_envelope);

    // Corrompre la version
    let mut corrupted = encoded.expose_secret().clone();
    corrupted[0] = 0; // Version invalide
    let corrupted_envelope = SecretBox::new(Box::new(corrupted));

    let result = decode_password_envelope(&corrupted_envelope);
    assert!(result.is_err(), "Should reject invalid version");

    // Corrompre avec une version future
    let mut future_version = encoded.expose_secret().clone();
    future_version[0] = 255; // Version future
    let future_envelope = SecretBox::new(Box::new(future_version));

    let result = decode_password_envelope(&future_envelope);
    assert!(result.is_err(), "Should reject future version");
}

#[tokio::test]
async fn test_truncated_envelope_fails_in_chain() {
    let crypto = CryptoServiceImpl::with_defaults();
    let password = "test_password";

    let account_key = generate_account_key().expect("Failed to generate account key");
    let password_envelope = wrap_account_key(
        &crypto,
        &SecretString::new(password.to_string().into()),
        &account_key,
    )
    .await
    .expect("Failed to wrap");

    let encoded = encode_password_envelope(&password_envelope);

    // Tronquer l'enveloppe
    let mut truncated = encoded.expose_secret().clone();
    truncated.truncate(10); // Trop court
    let truncated_envelope = SecretBox::new(Box::new(truncated));

    let result = decode_password_envelope(&truncated_envelope);
    assert!(result.is_err(), "Should reject truncated envelope");
}

#[tokio::test]
async fn test_chain_with_legacy_password_envelope() {
    // Tester que la chaîne fonctionne aussi avec l'ancien format (v1)
    use heelonvault_core::services::auth_service::{
        PasswordEnvelope::Legacy, encode_password_envelope,
    };
    use heelonvault_core::services::crypto_service::CryptoServiceImpl;

    let crypto = CryptoServiceImpl::with_defaults();
    let password = "test_password";

    // Créer un legacy envelope (v1) - la clé dérivée est utilisée directement comme master key
    let salt = crypto
        .generate_kdf_salt()
        .await
        .expect("Failed to generate salt");
    let master_key = crypto
        .derive_key(&SecretString::new(password.to_string().into()), &salt)
        .await
        .expect("Failed to derive key");

    // SecretBox<Vec<u8>> is not Clone: rebuild an independent copy for the envelope so
    // `master_key` stays available for the later comparison below.
    let master_key_copy = SecretBox::new(Box::new(master_key.expose_secret().clone()));
    let legacy_envelope = Legacy {
        salt,
        master_key: master_key_copy,
    };
    let encoded = encode_password_envelope(&legacy_envelope);

    // Décoder
    let decoded = decode_password_envelope(&encoded).expect("Failed to decode");
    assert!(
        matches!(decoded, Legacy { .. }),
        "Should be legacy envelope"
    );

    // Déverrouiller avec le bon mot de passe
    let result = unlock_password_envelope(
        &crypto,
        &decoded,
        &SecretString::new(password.to_string().into()),
    )
    .await
    .expect("Failed to unlock");

    assert!(result.is_some(), "Should unlock with correct password");
    let unlocked_key = result.expect("legacy envelope unlocked with the correct password");

    // La clé doit correspondre à la clé dérivée
    assert_eq!(unlocked_key.expose_secret(), master_key.expose_secret());

    // Faire échouer avec le mauvais mot de passe
    let wrong_result = unlock_password_envelope(
        &crypto,
        &decoded,
        &SecretString::new("wrong_password".to_string().into()),
    )
    .await
    .expect("Should not panic");

    assert!(
        wrong_result.is_none(),
        "Should not unlock with wrong password"
    );
}

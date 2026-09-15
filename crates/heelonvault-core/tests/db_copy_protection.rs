#![allow(clippy::disallowed_methods)]

//! Security tests for protection against database copy attacks.
//! Verifies that the database cannot be used on another machine without the appropriate keys.
//!
//! These tests exercise a real migrated SQLite database (via `sqlx::migrate::Migrator` against
//! the actual migrations, like `account_rekey_integration.rs`) and the real repositories/
//! services, instead of a hand-maintained copy of the schema — so "copy the DB file and try to
//! open it without the password" actually tests production behavior.

use heelonvault_core::models::Vault;
use heelonvault_core::models::secret_item::{BlobStorage, SecretItem, SecretType};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_core::services::account_key::generate_account_key;
use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
use heelonvault_core::services::auth_service::{
    AuthServiceImpl, decode_password_envelope, encode_password_envelope, unlock_password_envelope,
    wrap_account_key,
};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{CryptoService, CryptoServiceImpl, KdfConfig};
use heelonvault_core::services::pin_cache_service::{
    PIN_MAX_FAILED_ATTEMPTS, PinCache, PinUnlockError,
};
use heelonvault_core::services::vault_service::{
    VaultKeyEnvelopeRepository, deserialize_vault_key_envelope, serialize_vault_key_envelope,
};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

const USERNAME: &str = "alice";
const PASSWORD: &str = "correct_horse_battery_staple_123";
const VAULT_KEY: [u8; 32] = [7; 32];
const SECRET_PLAINTEXT: &[u8] = b"my_super_secret_data_123!";

fn fast_crypto() -> CryptoServiceImpl {
    CryptoServiceImpl::new(KdfConfig {
        memory_cost_kib: 1024,
        time_cost: 1,
        parallelism: 1,
        output_len: 32,
    })
}

fn secret(value: &str) -> SecretBox<Vec<u8>> {
    SecretBox::new(Box::new(value.as_bytes().to_vec()))
}

async fn open_pool(path: &Path) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .pragma("secure_delete", "ON"),
        )
        .await
        .expect("open sqlite pool");
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../heelonvault-app/migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .expect("load migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    pool
}

async fn encrypt_with(key: &[u8], plaintext: &[u8]) -> SecretBox<Vec<u8>> {
    let payload = fast_crypto()
        .encrypt(
            &SecretBox::new(Box::new(plaintext.to_vec())),
            &SecretBox::new(Box::new(key.to_vec())),
        )
        .await
        .expect("encrypt");
    // Real production code stores vault-key envelopes and secret blobs as `nonce ||
    // ciphertext` (via `serialize_vault_key_envelope` / secret_service's identical private
    // helper), not the version-prefixed `crypto_service::encode_envelope` format (a legacy
    // format `rekey_service.rs` still knows how to read).
    serialize_vault_key_envelope(&payload)
}

struct Seed {
    dir: TempDir,
    db_path: PathBuf,
    user_id: Uuid,
    vault_id: Uuid,
    secret_id: Uuid,
}

/// A single-user database with one vault and one secret, seeded through the real
/// bootstrap/repository code paths (not a hand-written schema).
async fn seed() -> Seed {
    let dir = TempDir::new().expect("temp dir");
    let db_path = dir.path().join("source.db");
    let pool = open_pool(&db_path).await;

    let backup = BackupServiceImpl::new();
    let phrase = backup
        .generate_recovery_key()
        .expect("recovery phrase")
        .recovery_phrase;
    let boot = bootstrap_first_admin_with_recovery(
        &SqlxUserRepository::new(pool.clone()),
        &AuthServiceImpl::new(fast_crypto()),
        &backup,
        &fast_crypto(),
        USERNAME,
        secret(PASSWORD),
        phrase,
    )
    .await
    .expect("bootstrap");

    let key = boot.master_key.expose_secret().clone();
    let vault_id = Uuid::new_v4();
    SqlxVaultRepository::new(pool.clone())
        .create_vault_with_envelope(
            &Vault {
                id: vault_id,
                owner_user_id: boot.user_id,
                name: "Perso".to_string(),
            },
            encrypt_with(&key, &VAULT_KEY).await,
        )
        .await
        .expect("vault");

    let secret_id = Uuid::new_v4();
    let item = SecretItem {
        id: secret_id,
        vault_id,
        secret_type: SecretType::Password,
        title: Some("test_secret".to_string()),
        metadata_json: None,
        tags: None,
        expires_at: None,
        created_at: None,
        modified_at: None,
        usage_count: 0,
        blob_storage: BlobStorage::Inline,
        secret_blob: SecretBox::new(Box::new(Vec::new())),
        deleted_at: None,
    };
    SqlxSecretRepository::new(pool.clone())
        .insert_secret_blob(&item, encrypt_with(&VAULT_KEY, SECRET_PLAINTEXT).await)
        .await
        .expect("secret");

    pool.close().await;
    Seed {
        dir,
        db_path,
        user_id: boot.user_id,
        vault_id,
        secret_id,
    }
}

async fn copy_db(seed: &Seed) -> SqlitePool {
    let copy_path = seed.dir.path().join("copy.db");
    std::fs::copy(&seed.db_path, &copy_path).expect("Failed to copy DB");
    open_pool(&copy_path).await
}

// ============================================================================
// Catégorie 2 : Tests de protection contre la copie sur autre machine
// ============================================================================

#[tokio::test]
async fn test_db_copy_requires_password() {
    let seed = seed().await;
    let copy_pool = copy_db(&seed).await;

    // Tenter de récupérer le secret directement depuis la copie, en contournant l'API
    // applicative (accès direct au repository, comme le ferait un attaquant avec le fichier).
    let stored = SqlxSecretRepository::new(copy_pool.clone())
        .list_by_vault_id(seed.vault_id)
        .await
        .expect("list secrets");
    let item = stored
        .iter()
        .find(|item| item.id == seed.secret_id)
        .expect("secret present in the copy");

    // Tenter de déchiffrer avec une clé aléatoire incorrecte (l'attaquant n'a pas le mot de
    // passe, donc pas la vault key).
    let crypto = CryptoServiceImpl::with_defaults();
    let wrong_key = SecretBox::new(Box::new(vec![0u8; 32]));
    let payload =
        deserialize_vault_key_envelope(&item.secret_blob).expect("Should decode envelope");
    let result = crypto.decrypt(&payload, &wrong_key).await;

    assert!(
        result.is_err(),
        "Should fail to decrypt with wrong key - DB copy is vulnerable!"
    );
}

#[tokio::test]
async fn test_db_copy_with_password_still_requires_account_key() {
    // Même en ayant copié la DB et en connaissant le mot de passe, il faut suivre toute la
    // chaîne (password -> account key -> vault key -> secret) pour lire le secret en clair.
    let seed = seed().await;
    let copy_pool = copy_db(&seed).await;

    let user_repo = SqlxUserRepository::new(copy_pool.clone());
    // Must match the KDF config `seed()` used to wrap the envelope (via fast_crypto()):
    // unlock re-derives the key with Argon2id, so a different KdfConfig here would yield a
    // different key from the same password and incorrectly look like a wrong password.
    let crypto = fast_crypto();

    let stored_envelope = user_repo
        .get_password_envelope_by_user_id(seed.user_id)
        .await
        .expect("query envelope")
        .expect("envelope present");
    let envelope =
        decode_password_envelope(&stored_envelope).expect("Should decode password envelope");

    let account_key = unlock_password_envelope(
        &crypto,
        &envelope,
        &SecretString::new(PASSWORD.to_string().into()),
    )
    .await
    .expect("Should unlock")
    .expect("Should unlock with correct password");

    let vault_envelope = SqlxVaultRepository::new(copy_pool.clone())
        .get_vault_key_envelope(seed.vault_id)
        .await
        .expect("query vault")
        .expect("vault envelope present");
    let vault_payload =
        deserialize_vault_key_envelope(&vault_envelope).expect("Should decode vault envelope");
    let vault_key = crypto
        .decrypt(&vault_payload, &account_key)
        .await
        .expect("Should decrypt vault key with account key");

    let stored = SqlxSecretRepository::new(copy_pool.clone())
        .list_by_vault_id(seed.vault_id)
        .await
        .expect("list secrets");
    let item = stored
        .iter()
        .find(|item| item.id == seed.secret_id)
        .expect("secret present");
    let secret_payload =
        deserialize_vault_key_envelope(&item.secret_blob).expect("Should decode secret envelope");
    let decrypted = crypto
        .decrypt(&secret_payload, &vault_key)
        .await
        .expect("Should decrypt secret with vault key");

    assert_eq!(
        decrypted.expose_secret(),
        SECRET_PLAINTEXT,
        "Decrypted secret should match original"
    );
}

#[tokio::test]
async fn test_db_copy_without_password_fails() {
    let seed = seed().await;
    let copy_pool = copy_db(&seed).await;

    let user_repo = SqlxUserRepository::new(copy_pool.clone());
    // Match `seed()`'s KDF config (see the comment in the previous test for why).
    let crypto = fast_crypto();

    let stored_envelope = user_repo
        .get_password_envelope_by_user_id(seed.user_id)
        .await
        .expect("query envelope")
        .expect("envelope present");
    let envelope = decode_password_envelope(&stored_envelope).expect("Should decode");

    // Essayer avec un mauvais mot de passe
    let result = unlock_password_envelope(
        &crypto,
        &envelope,
        &SecretString::new("wrong_password".to_string().into()),
    )
    .await
    .expect("Should not panic");
    assert!(result.is_none(), "Should not unlock with wrong password");

    // Sans l'account key, on ne peut pas déchiffrer la vault key
    let vault_envelope = SqlxVaultRepository::new(copy_pool.clone())
        .get_vault_key_envelope(seed.vault_id)
        .await
        .expect("query vault")
        .expect("vault envelope present");
    let vault_payload = deserialize_vault_key_envelope(&vault_envelope).expect("Should decode");
    let wrong_key = SecretBox::new(Box::new(vec![0u8; 32]));
    let result = crypto.decrypt(&vault_payload, &wrong_key).await;
    assert!(
        result.is_err(),
        "Should fail to decrypt vault key without account key"
    );

    // Et on ne peut pas déchiffrer le secret sans la vault key
    let stored = SqlxSecretRepository::new(copy_pool.clone())
        .list_by_vault_id(seed.vault_id)
        .await
        .expect("list secrets");
    let item = stored
        .iter()
        .find(|item| item.id == seed.secret_id)
        .expect("secret present");
    let secret_payload = deserialize_vault_key_envelope(&item.secret_blob).expect("Should decode");
    let result = crypto.decrypt(&secret_payload, &wrong_key).await;
    assert!(
        result.is_err(),
        "Should fail to decrypt secret without vault key"
    );
}

// ============================================================================
// Catégorie 2.2 : Tests de binding au contexte d'exécution
// ============================================================================

#[tokio::test]
async fn test_pin_cache_bound_to_user_id() {
    let master_key = SecretBox::new(Box::new(vec![0xAB; 32]));
    let user_id_1 = Uuid::new_v4();
    let user_id_2 = Uuid::new_v4();

    let mut cache = PinCache::wrap(&master_key, "1234", user_id_1).expect("Should create cache");
    assert_eq!(cache.user_id(), user_id_1);

    let result = cache.try_unwrap("1234");
    assert!(result.is_ok(), "Should unlock with correct PIN");
    assert_ne!(
        cache.user_id(),
        user_id_2,
        "Cache should be bound to user_id_1, not user_id_2"
    );

    let mut cache_2 =
        PinCache::wrap(&master_key, "1234", user_id_2).expect("Should create second cache");
    assert_ne!(
        cache.user_id(),
        cache_2.user_id(),
        "Caches should have different user_ids"
    );

    let result_2 = cache_2.try_unwrap("1234");
    assert!(
        result_2.is_ok(),
        "Second cache should unlock with correct PIN"
    );
}

#[tokio::test]
async fn test_pin_cache_timeout_protection() {
    let master_key = SecretBox::new(Box::new(vec![0xAB; 32]));
    let user_id = Uuid::new_v4();

    let cache = PinCache::wrap(&master_key, "1234", user_id).expect("Should create cache");

    assert!(
        !cache.is_expired(Duration::from_secs(3600)),
        "Cache should not be expired immediately"
    );
    assert!(
        cache.is_expired(Duration::from_nanos(0)),
        "Cache should be expired with zero timeout"
    );

    let remaining = cache.remaining(Duration::from_secs(3600));
    assert!(
        remaining <= Duration::from_secs(3600),
        "Remaining time should be <= timeout"
    );
    assert!(remaining > Duration::ZERO, "Remaining time should be > 0");
}

#[tokio::test]
async fn test_pin_cache_max_attempts() {
    let master_key = SecretBox::new(Box::new(vec![0xAB; 32]));
    let user_id = Uuid::new_v4();

    let mut cache = PinCache::wrap(&master_key, "1234", user_id).expect("Should create cache");

    // Essayer PIN_MAX_FAILED_ATTEMPTS fois avec le mauvais PIN. `failed_attempts` est un champ
    // privé de PinCache (volontairement, il ne s'expose qu'à travers l'API publique) : on suit
    // sa progression via le `remaining` porté par PinUnlockError::WrongPin plutôt que d'y
    // accéder directement.
    for i in 0..PIN_MAX_FAILED_ATTEMPTS {
        let result = cache.try_unwrap("0000");
        let is_last_attempt = i + 1 == PIN_MAX_FAILED_ATTEMPTS;
        if is_last_attempt {
            assert!(
                matches!(result, Err(PinUnlockError::Exhausted)),
                "attempt {i} (the last one) should report Exhausted, got {result:?}"
            );
        } else {
            let expected_remaining = PIN_MAX_FAILED_ATTEMPTS - (i + 1);
            assert!(
                matches!(result, Err(PinUnlockError::WrongPin { remaining }) if remaining == expected_remaining),
                "attempt {i} should report {expected_remaining} remaining, got {result:?}"
            );
        }
    }

    // Le cache doit être épuisé
    assert!(
        cache.is_exhausted(),
        "Cache should be exhausted after max attempts"
    );

    // Toute nouvelle tentative doit échouer, même avec le bon PIN
    let result = cache.try_unwrap("1234");
    assert!(
        matches!(result, Err(PinUnlockError::Exhausted)),
        "Should fail even with correct PIN after exhaustion, got {result:?}"
    );
}

#[tokio::test]
async fn test_pin_cache_different_pins_for_different_users() {
    let master_key_1 = SecretBox::new(Box::new(vec![0xAB; 32]));
    let master_key_2 = SecretBox::new(Box::new(vec![0xCD; 32]));
    let user_id_1 = Uuid::new_v4();
    let user_id_2 = Uuid::new_v4();

    let mut cache_1 =
        PinCache::wrap(&master_key_1, "1234", user_id_1).expect("Should create cache 1");
    let mut cache_2 =
        PinCache::wrap(&master_key_2, "5678", user_id_2).expect("Should create cache 2");

    assert!(
        cache_1.try_unwrap("1234").is_ok(),
        "Cache 1 should open with PIN 1234"
    );
    assert!(
        cache_2.try_unwrap("5678").is_ok(),
        "Cache 2 should open with PIN 5678"
    );

    assert!(
        cache_1.try_unwrap("5678").is_err(),
        "Cache 1 should not open with PIN 5678"
    );
    assert!(
        cache_2.try_unwrap("1234").is_err(),
        "Cache 2 should not open with PIN 1234"
    );
}

// ============================================================================
// Tests supplémentaires : Protection contre le replay cross-user
// ============================================================================

#[tokio::test]
async fn test_cross_user_replay_attack_fails() {
    // Simuler une attaque où un attaquant essaie de réutiliser le password_envelope d'un
    // utilisateur pour un autre.
    let dir = TempDir::new().expect("temp dir");
    let pool = open_pool(&dir.path().join("test.db")).await;
    let crypto = CryptoServiceImpl::with_defaults();

    let user_id_1 = Uuid::new_v4();
    let user_id_2 = Uuid::new_v4();
    let password_1 = "password_user_1_long_enough";
    let password_2 = "password_user_2_long_enough";

    let account_key_1 = generate_account_key().expect("Failed to generate account key 1");
    let account_key_2 = generate_account_key().expect("Failed to generate account key 2");

    let password_envelope_1 = wrap_account_key(
        &crypto,
        &SecretString::new(password_1.to_string().into()),
        &account_key_1,
    )
    .await
    .expect("Failed to wrap for user 1");
    let password_envelope_2 = wrap_account_key(
        &crypto,
        &SecretString::new(password_2.to_string().into()),
        &account_key_2,
    )
    .await
    .expect("Failed to wrap for user 2");

    let user_repo = SqlxUserRepository::new(pool.clone());
    user_repo
        .create_user_db(
            user_id_1,
            "user1",
            &heelonvault_core::models::UserRole::Admin,
        )
        .await
        .expect("create user 1");
    user_repo
        .update_password_envelope(user_id_1, encode_password_envelope(&password_envelope_1))
        .await
        .expect("store envelope 1");
    user_repo
        .create_user_db(
            user_id_2,
            "user2",
            &heelonvault_core::models::UserRole::User,
        )
        .await
        .expect("create user 2");
    user_repo
        .update_password_envelope(user_id_2, encode_password_envelope(&password_envelope_2))
        .await
        .expect("store envelope 2");

    let stored_envelope_1 = user_repo
        .get_password_envelope_by_user_id(user_id_1)
        .await
        .expect("query envelope")
        .expect("envelope present");
    let decoded = decode_password_envelope(&stored_envelope_1).expect("Should decode");

    // Essayer de l'utiliser avec le mot de passe de user_2
    let result = unlock_password_envelope(
        &crypto,
        &decoded,
        &SecretString::new(password_2.to_string().into()),
    )
    .await
    .expect("Should not panic");

    assert!(
        result.is_none(),
        "Should not unlock user_1's envelope with user_2's password"
    );
}

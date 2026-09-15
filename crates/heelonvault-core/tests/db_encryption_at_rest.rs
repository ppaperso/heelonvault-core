#![allow(clippy::disallowed_methods)]

//! Security tests for encryption at rest.
//! Verifies that ALL sensitive data is encrypted before storage in the database.
//!
//! These tests run against a real migrated SQLite database (via `sqlx::migrate::Migrator`
//! against the actual migrations, like `account_rekey_integration.rs`) and the real
//! repositories, instead of a hand-maintained copy of the schema — so a passing test actually
//! says something about the production schema.

use heelonvault_core::models::Vault;
use heelonvault_core::models::secret_item::{BlobStorage, SecretItem, SecretType};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
use heelonvault_core::services::auth_service::AuthServiceImpl;
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{
    CryptoService, CryptoServiceImpl, KdfConfig, NONCE_LEN,
};
use heelonvault_core::services::vault_service::{
    VaultKeyEnvelopeRepository, deserialize_vault_key_envelope, serialize_vault_key_envelope,
};
use secrecy::{ExposeSecret, SecretBox};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const USERNAME: &str = "test_user";
const PASSWORD: &str = "test_password_1234567890";
const VAULT_NAME: &str = "test_vault";
const VAULT_KEY: [u8; 32] = [7; 32];

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

fn file_contains(path: &Path, needle: &[u8]) -> bool {
    std::fs::read(path)
        .expect("read database file")
        .windows(needle.len())
        .any(|window| window == needle)
}

struct Env {
    #[allow(dead_code)] // keeps the TempDir (and its directory) alive for the test's duration
    dir: TempDir,
    db_path: PathBuf,
    pool: SqlitePool,
    user_id: Uuid,
    vault_id: Uuid,
}

/// A real user (bootstrapped through `bootstrap_first_admin_with_recovery`, exactly like
/// production startup) with one empty vault, on a fully migrated database.
async fn setup() -> Env {
    let dir = TempDir::new().expect("temp dir");
    let db_path = dir.path().join("test.db");
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
                name: VAULT_NAME.to_string(),
            },
            encrypt_with(&key, &VAULT_KEY).await,
        )
        .await
        .expect("vault");

    Env {
        dir,
        db_path,
        pool,
        user_id: boot.user_id,
        vault_id,
    }
}

async fn insert_secret(pool: &SqlitePool, vault_id: Uuid, plaintext: &str) -> Uuid {
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
        .insert_secret_blob(&item, encrypt_with(&VAULT_KEY, plaintext.as_bytes()).await)
        .await
        .expect("insert secret");
    secret_id
}

async fn secret_blob(pool: &SqlitePool, vault_id: Uuid, secret_id: Uuid) -> Vec<u8> {
    SqlxSecretRepository::new(pool.clone())
        .list_by_vault_id(vault_id)
        .await
        .expect("list secrets")
        .into_iter()
        .find(|item| item.id == secret_id)
        .expect("secret present")
        .secret_blob
        .expose_secret()
        .clone()
}

async fn vault_key_envelope(pool: &SqlitePool, vault_id: Uuid) -> Vec<u8> {
    SqlxVaultRepository::new(pool.clone())
        .get_vault_key_envelope(vault_id)
        .await
        .expect("query vault")
        .expect("vault envelope present")
        .expose_secret()
        .clone()
}

async fn password_envelope(pool: &SqlitePool, user_id: Uuid) -> Vec<u8> {
    SqlxUserRepository::new(pool.clone())
        .get_password_envelope_by_user_id(user_id)
        .await
        .expect("query envelope")
        .expect("envelope present")
        .expose_secret()
        .clone()
}

// ============================================================================
// Catégorie 1 : Tests de chiffrement des données au repos
// ============================================================================

#[tokio::test]
async fn test_secret_blob_is_encrypted_in_db() {
    let env = setup().await;
    let plaintext = "mon_secret_super_secret_123";
    let secret_id = insert_secret(&env.pool, env.vault_id, plaintext).await;

    let blob = secret_blob(&env.pool, env.vault_id, secret_id).await;

    assert!(
        !blob
            .windows(plaintext.len())
            .any(|w| w == plaintext.as_bytes()),
        "Plaintext found in encrypted blob! This is a critical security issue!"
    );
    // Real secret blobs are `nonce(12) || ciphertext` (see `secret_service.rs`'s private
    // `serialize_payload`/`deserialize_payload`, and `serialize_vault_key_envelope`, whose
    // logic is byte-for-byte identical) — no version byte, unlike `crypto_service::
    // encode_envelope`. A blob no longer than the bare nonce couldn't hold any ciphertext.
    assert!(
        blob.len() > NONCE_LEN,
        "Blob too short to contain a nonce and ciphertext"
    );
}

#[tokio::test]
async fn test_vault_key_is_encrypted_in_db() {
    let env = setup().await;
    let envelope = vault_key_envelope(&env.pool, env.vault_id).await;

    assert!(
        envelope.len() > NONCE_LEN,
        "Vault key envelope too short to contain a nonce and ciphertext"
    );
    // A raw AES-256 key would be exactly 32 bytes; an encrypted envelope must be longer
    // (nonce + ciphertext + auth tag, no version byte — see `serialize_vault_key_envelope`).
    assert!(
        envelope.len() > 32,
        "Vault key envelope appears to be unencrypted 32-byte key!"
    );
}

#[tokio::test]
async fn test_password_envelope_is_not_plaintext() {
    let env = setup().await;
    let envelope = password_envelope(&env.pool, env.user_id).await;

    // Not a simple hash (too short)
    assert!(
        envelope.len() > 64,
        "Password envelope too short ({} bytes) - might be plain hash! Expected > 64 bytes",
        envelope.len()
    );

    assert!(envelope.len() >= 5, "Envelope header missing");
    let version = envelope[0];
    assert!(
        version == 1 || version == 2,
        "Invalid envelope version: {}. Expected 1 (legacy) or 2 (account key)",
        version
    );
}

#[tokio::test]
async fn test_all_envelopes_are_structured_ciphertext_not_raw_bytes() {
    // Only `users.password_envelope` carries a version byte (see
    // `test_password_envelope_structure_v2`, and `auth_service.rs`'s own versioning).
    // `secrets.secret_blob` and `vaults.vault_key_envelope` use the shorter `nonce ||
    // ciphertext` form — this test checks that every one of them is at least long enough to
    // hold real structure (nonce + auth tag), i.e. isn't an empty or truncated placeholder.
    let env = setup().await;
    let secret_id = insert_secret(&env.pool, env.vault_id, "test").await;

    let nonce_ciphertext_blobs = [
        (
            "secrets.secret_blob",
            secret_blob(&env.pool, env.vault_id, secret_id).await,
        ),
        (
            "vaults.vault_key_envelope",
            vault_key_envelope(&env.pool, env.vault_id).await,
        ),
    ];
    for (label, blob) in nonce_ciphertext_blobs {
        assert!(
            blob.len() > NONCE_LEN,
            "{label} too short to contain a nonce and ciphertext: {} bytes",
            blob.len()
        );
    }

    let password_envelope = password_envelope(&env.pool, env.user_id).await;
    assert!(
        !password_envelope.is_empty(),
        "Empty users.password_envelope"
    );
    assert!(
        password_envelope[0] == 1 || password_envelope[0] == 2,
        "Invalid version in users.password_envelope: got {}",
        password_envelope[0]
    );
}

#[tokio::test]
async fn test_envelope_nonce_uniqueness() {
    let env = setup().await;

    let secret_ids: Vec<Uuid> = {
        let mut ids = Vec::with_capacity(100);
        for i in 0..100 {
            ids.push(insert_secret(&env.pool, env.vault_id, &format!("test-{i}")).await);
        }
        ids
    };

    let mut nonces = Vec::new();
    for id in &secret_ids {
        let blob = secret_blob(&env.pool, env.vault_id, *id).await;
        assert!(
            blob.len() > NONCE_LEN,
            "Blob too short to contain nonce for secret {id}"
        );
        // `nonce || ciphertext`, no leading version byte — see `serialize_vault_key_envelope`.
        nonces.push(blob[0..NONCE_LEN].to_vec());
    }

    use std::collections::HashSet;
    let mut unique_nonces = HashSet::new();
    for (i, nonce) in nonces.iter().enumerate() {
        assert!(
            unique_nonces.insert(nonce),
            "Duplicate nonce found at index {i}! This is a critical security issue! Nonce: {nonce:?}"
        );
    }
}

#[tokio::test]
async fn test_password_envelope_structure_v2() {
    let env = setup().await;
    let envelope = password_envelope(&env.pool, env.user_id).await;

    // V2 envelope structure: version(1) || salt_len(2) || body_len(2) || salt(salt_len) || nonce(12) || ciphertext
    assert_eq!(
        envelope[0], 2,
        "Expected password envelope version 2 (account key)"
    );

    let salt_len = u16::from_be_bytes([envelope[1], envelope[2]]) as usize;
    let body_len = u16::from_be_bytes([envelope[3], envelope[4]]) as usize;
    let expected_len = 5 + salt_len + body_len;

    assert_eq!(
        envelope.len(),
        expected_len,
        "Password envelope length mismatch. Expected {}, got {}",
        expected_len,
        envelope.len()
    );
    assert!(
        body_len >= 12,
        "Password envelope body too short to contain nonce"
    );
}

#[tokio::test]
async fn test_secret_blob_contains_valid_envelope() {
    let env = setup().await;
    let secret_id = insert_secret(&env.pool, env.vault_id, "test_secret_data").await;
    let blob = SecretBox::new(Box::new(
        secret_blob(&env.pool, env.vault_id, secret_id).await,
    ));

    let payload =
        deserialize_vault_key_envelope(&blob).expect("Failed to decode secret blob envelope");
    assert_eq!(payload.nonce.len(), 12, "Nonce must be 12 bytes");
    assert!(
        !payload.ciphertext.expose_secret().is_empty(),
        "Ciphertext must not be empty"
    );
}

#[tokio::test]
async fn test_vault_key_envelope_contains_valid_envelope() {
    let env = setup().await;
    let blob = SecretBox::new(Box::new(vault_key_envelope(&env.pool, env.vault_id).await));

    let payload =
        deserialize_vault_key_envelope(&blob).expect("Failed to decode vault key envelope");
    assert_eq!(payload.nonce.len(), 12, "Nonce must be 12 bytes");
    assert!(
        !payload.ciphertext.expose_secret().is_empty(),
        "Ciphertext must not be empty"
    );
}

#[tokio::test]
async fn test_direct_db_read_reveals_no_sensitive_data() {
    // Ce test vérifie que même avec un accès direct aux repositories (contournant l'UI/API,
    // comme le ferait un attaquant ayant une copie du fichier), on ne peut pas extraire de
    // données sensibles sans les clés.
    let env = setup().await;
    let secret_plaintext = "my_super_secret_password_123!";
    let secret_id = insert_secret(&env.pool, env.vault_id, secret_plaintext).await;

    let password_envelope = password_envelope(&env.pool, env.user_id).await;
    let vault_key_envelope = vault_key_envelope(&env.pool, env.vault_id).await;
    let secret_blob = secret_blob(&env.pool, env.vault_id, secret_id).await;

    let sensitive_strings = [PASSWORD, secret_plaintext, USERNAME, VAULT_NAME];
    let all_blobs = [
        ("password_envelope", &password_envelope),
        ("vault_key_envelope", &vault_key_envelope),
        ("secret_blob", &secret_blob),
    ];

    for (blob_name, blob) in all_blobs {
        for sensitive in sensitive_strings {
            assert!(
                !blob
                    .windows(sensitive.len())
                    .any(|w| w == sensitive.as_bytes()),
                "Sensitive data '{sensitive}' found in {blob_name}! This is a critical security issue!"
            );
        }
    }
}

#[tokio::test]
async fn test_secret_plaintext_never_appears_in_the_raw_database_file() {
    // La garantie la plus directe pour "encryption at rest" : les octets bruts du fichier
    // .db sur disque, pas seulement les valeurs relues via SQL/les repositories.
    let env = setup().await;
    let secret_plaintext = "raw_file_scan_secret_456!";
    insert_secret(&env.pool, env.vault_id, secret_plaintext).await;
    env.pool.close().await;

    // The username is legitimately stored in clear (it must stay queryable for login) — only
    // the password and the secret's plaintext are actual secrets here.
    for sensitive in [PASSWORD, secret_plaintext] {
        assert!(
            !file_contains(&env.db_path, sensitive.as_bytes()),
            "'{sensitive}' must never appear in clear in the raw database file"
        );
    }
}

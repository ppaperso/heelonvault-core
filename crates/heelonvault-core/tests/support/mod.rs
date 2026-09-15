//! Real-database fixtures shared by the security/privacy integration tests: a migrated SQLite
//! file, a bootstrapped account, and helpers that scan what actually lands on disk.
//!
//! Each test binary includes this module with `mod support;` and uses only part of it.
#![allow(dead_code)] // shared across test binaries, each one uses a different subset
#![allow(clippy::disallowed_methods)] // test fixtures: a failed setup must abort the test

use std::path::{Path, PathBuf};

use heelonvault_core::models::Vault;
use heelonvault_core::models::secret_item::{BlobStorage, SecretItem, SecretType};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::SqlxUserRepository;
use heelonvault_core::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
use heelonvault_core::services::auth_service::AuthServiceImpl;
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{CryptoService, CryptoServiceImpl, KdfConfig};
use heelonvault_core::services::vault_service::serialize_vault_key_envelope;
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use uuid::Uuid;

pub const USERNAME: &str = "alice";
pub const PASSWORD: &str = "Correct-Horse-Battery-Staple-2026!";

/// Argon2id with minimal cost: the tests check behavior, not the production work factor
/// (asserted separately in `bruteforce_resistance.rs`).
pub fn fast_crypto() -> CryptoServiceImpl {
    CryptoServiceImpl::new(KdfConfig {
        memory_cost_kib: 1024,
        time_cost: 1,
        parallelism: 1,
        output_len: 32,
    })
}

pub fn secret(value: &str) -> SecretBox<Vec<u8>> {
    SecretBox::new(Box::new(value.as_bytes().to_vec()))
}

/// Same hardening pragmas as the application (`hardened_sqlite_options` in heelonvault-app).
pub async fn open_pool(path: &Path) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .pragma("secure_delete", "ON")
                .pragma("temp_store", "MEMORY"),
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

/// Vault keys and secret blobs are stored as `nonce || ciphertext` in production.
pub async fn encrypt_with(key: &[u8], plaintext: &[u8]) -> SecretBox<Vec<u8>> {
    let payload = fast_crypto()
        .encrypt(
            &SecretBox::new(Box::new(plaintext.to_vec())),
            &SecretBox::new(Box::new(key.to_vec())),
        )
        .await
        .expect("encrypt");
    serialize_vault_key_envelope(&payload)
}

pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// Scans the database file and its `-wal`/`-shm`/`-journal` side files, which hold pages the
/// main file does not show yet.
pub fn db_files_contain(db_path: &Path, needle: &[u8]) -> bool {
    let mut candidates = vec![db_path.to_path_buf()];
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut side = db_path.as_os_str().to_owned();
        side.push(suffix);
        candidates.push(PathBuf::from(side));
    }
    candidates
        .iter()
        .filter_map(|path| std::fs::read(path).ok())
        .any(|bytes| contains(&bytes, needle))
}

pub struct Account {
    pub dir: TempDir,
    pub db_path: PathBuf,
    pub pool: SqlitePool,
    pub user_id: Uuid,
    pub account_key: Vec<u8>,
    pub recovery_phrase: SecretString,
}

/// A migrated database with one account bootstrapped exactly like first start-up.
pub async fn bootstrap_account() -> Account {
    let dir = TempDir::new().expect("temp dir");
    let db_path = dir.path().join("vault.db");
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
    Account {
        dir,
        db_path,
        pool,
        user_id: boot.user_id,
        account_key: boot.master_key.expose_secret().clone(),
        recovery_phrase: boot.recovery_phrase,
    }
}

/// Creates a vault owned by `owner` whose key is wrapped under `account_key`.
pub async fn create_vault(
    pool: &SqlitePool,
    owner: Uuid,
    account_key: &[u8],
    vault_key: &[u8; 32],
) -> Uuid {
    let vault_id = Uuid::new_v4();
    SqlxVaultRepository::new(pool.clone())
        .create_vault_with_envelope(
            &Vault {
                id: vault_id,
                owner_user_id: owner,
                name: "Perso".to_string(),
            },
            encrypt_with(account_key, vault_key).await,
        )
        .await
        .expect("vault");
    vault_id
}

/// Inserts a secret encrypted under `vault_key`; returns its id and the stored ciphertext.
pub async fn insert_secret(
    pool: &SqlitePool,
    vault_id: Uuid,
    vault_key: &[u8; 32],
    plaintext: &[u8],
) -> (Uuid, Vec<u8>) {
    let item = SecretItem {
        id: Uuid::new_v4(),
        vault_id,
        secret_type: SecretType::Password,
        title: Some("entry".to_string()),
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
    let blob = encrypt_with(vault_key, plaintext).await;
    let stored = blob.expose_secret().clone();
    SqlxSecretRepository::new(pool.clone())
        .insert_secret_blob(&item, blob)
        .await
        .expect("secret");
    (item.id, stored)
}

#[cfg(unix)]
pub fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("file metadata")
        .permissions()
        .mode()
        & 0o777
}

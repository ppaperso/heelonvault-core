//! Frozen synthetic ciphertext and schema baseline, verified against 2.1.1.
//! Never regenerate these vectors to accommodate a format change.

use std::path::Path;
use std::sync::Arc;

use heelonvault_core::models::{BlobStorage, SecretType};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::SqlxUserRepository;
use heelonvault_core::services::audit_log_service::NoOpAuditLogService;
use heelonvault_core::services::backup_application_service::{
    BackupApplicationService, BackupApplicationServiceImpl,
};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::CryptoServiceImpl;
use heelonvault_core::services::secret_service::{SecretService, SecretServiceImpl};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn sqlite_runtime_includes_wal_page_size_fix() -> TestResult {
    tokio::runtime::Runtime::new()?.block_on(async {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        let version: String = sqlx::query_scalar("SELECT sqlite_version()")
            .fetch_one(&pool)
            .await?;
        let components: Vec<u32> = version
            .split('.')
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        assert_eq!(components.len(), 3, "unexpected SQLite version: {version}");
        assert!(
            components.as_slice() >= [3, 54, 0].as_slice(),
            "SQLite {version} lacks the RUSTSEC-2026-0459 fix"
        );
        pool.close().await;
        Ok(())
    })
}

const SCHEMA_VERSION: i64 = 19;
const SCHEMA_SHA256: &str = "cdb8df1bfdd6f5acd181e9efc54ad351aead7183546d24b1a68db7739369759b";
const USER_ID: Uuid = Uuid::from_u128(1);
const VAULT_ID: Uuid = Uuid::from_u128(2);
const KEY: [u8; 32] = [0x42; 32];
const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

// Public test values only. Production secret blobs use nonce || AES-GCM ciphertext/tag.
const VECTORS: [(SecretType, &str, &[u8], &str); 4] = [
    (
        SecretType::Password,
        "password",
        b"synthetic-password-2.1.1!",
        "0101010101010101010101015ed0d873fc31f5a4bc7b99ad9464cca4d8e7d1f1a39df93269b19c757e648dcf1d8c9f4cb5a168fc9b",
    ),
    (
        SecretType::ApiToken,
        "api_token",
        b"synthetic-api-token",
        "020202020202020202020202819c6876b49923eace87f241a3cbc790a47b7f0ec6ce145a84ead55c00b9975c282a98",
    ),
    (
        SecretType::SshKey,
        "ssh_key",
        b"-----BEGIN SYNTHETIC SSH KEY-----",
        "030303030303030303030303fc411f3df14876fde22a196627ca5f2783404c81637100e41801b043f2bf9f349fa8c67135d7aeb7937246baaaa363ab6e",
    ),
    (
        SecretType::SecureDocument,
        "secure_document",
        b"/synthetic/documents/reference.txt",
        "0404040404040404040404040964da2ae9140f71a1ac8da3f2d595d3e4493bfa6f522a775bbb791166cb05846d32096a669ec037c73113ec1b54a69f705c",
    ),
];

async fn open_pool(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
}

async fn verify_secrets(pool: &SqlitePool) -> TestResult {
    let repository = SqlxSecretRepository::new(pool.clone());
    let service = SecretServiceImpl::new(
        SqlxSecretRepository::new(pool.clone()),
        CryptoServiceImpl::with_defaults(),
        Arc::new(NoOpAuditLogService),
    );
    let items = repository.list_by_vault_id(VAULT_ID).await?;
    assert_eq!(items.len(), VECTORS.len());
    for (index, (kind, _, plaintext, ciphertext)) in VECTORS.iter().enumerate() {
        let id = Uuid::from_u128(10 + u128::try_from(index)?);
        let item = repository.get_by_id(id).await?.ok_or("missing fixture")?;
        assert_eq!(
            std::mem::discriminant(&item.secret_type),
            std::mem::discriminant(kind)
        );
        assert_eq!(item.secret_blob.expose_secret(), &hex::decode(ciphertext)?);
        assert_eq!(
            matches!(item.blob_storage, BlobStorage::File),
            matches!(kind, SecretType::SecureDocument)
        );
        let decrypted = service
            .get_secret(id, SecretBox::new(Box::new(KEY.to_vec())))
            .await?;
        assert_eq!(
            decrypted.secret_value.expose_secret().as_slice(),
            *plaintext
        );
        assert!(
            service
                .get_secret(id, SecretBox::new(Box::new(vec![0x43; 32])))
                .await
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn legacy_database_survives_migrations_and_backup_restore() -> TestResult {
    tokio::runtime::Runtime::new()?.block_on(async {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("legacy.db");
        let pool = open_pool(&path).await?;
        let migrations_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../heelonvault-app/migrations");
        let migrations = Migrator::new(migrations_path.as_path()).await?;
        let baseline = migrations
            .iter()
            .filter(|migration| migration.version <= SCHEMA_VERSION)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(baseline.len(), usize::try_from(SCHEMA_VERSION)?);
        let mut digest = Sha256::new();
        for migration in &baseline {
            digest.update(migration.sql.as_str().as_bytes());
        }
        assert_eq!(hex::encode(digest.finalize()), SCHEMA_SHA256);
        Migrator::with_migrations(baseline).run(&pool).await?;

        sqlx::query(
            "INSERT INTO users (id, username, role) VALUES (?1, 'synthetic-owner', 'admin')",
        )
        .bind(USER_ID.to_string())
        .execute(&pool)
        .await?;
        sqlx::query(
            "INSERT INTO vaults (id, owner_user_id, name) VALUES (?1, ?2, 'Synthetic baseline')",
        )
        .bind(VAULT_ID.to_string())
        .bind(USER_ID.to_string())
        .execute(&pool)
        .await?;
        for (index, (kind, stored_kind, _, ciphertext)) in VECTORS.iter().enumerate() {
            let is_document = matches!(kind, SecretType::SecureDocument);
            let ciphertext = hex::decode(ciphertext)?;
            sqlx::query(
                "INSERT INTO secret_items
                 (id, vault_id, secret_type, title, blob_storage, secret_blob, file_blob_ref)
                 VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6)",
            )
            .bind(Uuid::from_u128(10 + u128::try_from(index)?).to_string())
            .bind(VAULT_ID.to_string())
            .bind(stored_kind)
            .bind(if is_document { "file" } else { "inline" })
            .bind((!is_document).then_some(ciphertext.as_slice()))
            .bind(is_document.then_some(ciphertext.as_slice()))
            .execute(&pool)
            .await?;
        }

        migrations.run(&pool).await?;
        verify_secrets(&pool).await?;

        let backup_path = directory.path().join("baseline.hvb");
        let restored_path = directory.path().join("restored.db");
        let phrase = SecretString::new(PHRASE.into());
        let backups = BackupApplicationServiceImpl::new(
            SqlxUserRepository::new(pool.clone()),
            BackupServiceImpl::new(),
            pool.clone(),
        );
        let exported = backups
            .export_backup_secured(USER_ID, &path, &backup_path, &phrase)
            .await?;
        let restored = BackupServiceImpl::new().import_hvb_with_recovery_key(
            &backup_path,
            &phrase,
            &restored_path,
        )?;
        assert_eq!(exported.sha256_hex, restored.sha256_hex);
        let restored_pool = open_pool(&restored_path).await?;
        migrations.run(&restored_pool).await?;
        verify_secrets(&restored_pool).await?;
        restored_pool.close().await;
        pool.close().await;
        Ok(())
    })
}

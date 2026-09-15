#![allow(clippy::disallowed_methods)]

//! Security tests for backup protection.
//! Verifies that backups are protected and cannot be restored without authorization.
//!
//! Authorization-only coverage (admin-only export/restore, `NotFound` for a missing actor,
//! rotation tickets, WAL-snapshot behavior) is already covered by
//! `backup_security_integration.rs` and the `#[cfg(test)]` module in
//! `src/services/backup_application_service.rs` — duplicating it here added nothing. This file
//! instead covers integration-level guarantees not exercised there: a real round trip through
//! the actual `BackupServiceImpl` (real Argon2id + AES-GCM, not `tests/common`'s
//! `StubBackupService`, whose `verify_recovery_phrase` always returns `Ok(true)`), confirming
//! that a tampered backup or a wrong recovery phrase is genuinely rejected, and that the
//! exported file never contains the plaintext secret or password.

use heelonvault_core::errors::{AppError, RecoveryFailure};
use heelonvault_core::models::Vault;
use heelonvault_core::models::secret_item::{BlobStorage, SecretItem, SecretType};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
use heelonvault_core::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{CryptoService, CryptoServiceImpl, KdfConfig};
use heelonvault_core::services::vault_service::{
    VaultKeyEnvelopeRepository, deserialize_vault_key_envelope, serialize_vault_key_envelope,
};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;

const USERNAME: &str = "alice";
const PASSWORD: &str = "the-backup-owner-password-42";
const VAULT_KEY: [u8; 32] = [7; 32];
const SECRET_PLAINTEXT: &[u8] = b"backup_roundtrip_secret_789!";

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
    // Real production code stores both vault-key envelopes and secret blobs as
    // `nonce || ciphertext` (via `serialize_vault_key_envelope` / `secret_service`'s own
    // identical private helper) — not the version-prefixed `crypto_service::encode_envelope`
    // format, which is only a legacy format `rekey_service.rs` still knows how to read.
    serialize_vault_key_envelope(&payload)
}

struct Seed {
    dir: TempDir,
    db_path: PathBuf,
    user_id: Uuid,
    vault_id: Uuid,
    recovery_phrase: SecretString,
}

/// A real single-user database (bootstrap, one vault, one secret) on a fully migrated
/// SQLite file, ready to be exported through the real `BackupServiceImpl`.
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

    let item = SecretItem {
        id: Uuid::new_v4(),
        vault_id,
        secret_type: SecretType::Password,
        title: Some("backup test secret".to_string()),
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
        recovery_phrase: boot.recovery_phrase,
    }
}

fn file_contains(path: &Path, needle: &[u8]) -> bool {
    std::fs::read(path)
        .expect("read backup file")
        .windows(needle.len())
        .any(|window| window == needle)
}

// ============================================================================
// Catégorie 6 : Tests de protection des backups
// ============================================================================

#[tokio::test]
async fn test_backup_roundtrip_restores_the_real_secret_and_never_leaks_it_in_clear() {
    let seed = seed().await;
    let backup = BackupServiceImpl::new();
    let hvb_path = seed.dir.path().join("export.hvb");
    let restored_path = seed.dir.path().join("restored.db");

    let metadata = backup
        .export_hvb_with_recovery_key(&seed.db_path, &hvb_path, &seed.recovery_phrase)
        .expect("export should succeed");
    assert!(
        metadata.plaintext_size > 0,
        "exported metadata should report a non-zero size"
    );

    // Le fichier exporté ne doit jamais contenir le secret ni le mot de passe en clair —
    // contrairement au StubBackupService, BackupServiceImpl chiffre réellement le contenu.
    assert!(
        !file_contains(&hvb_path, SECRET_PLAINTEXT),
        "the exported backup must never contain the secret's plaintext"
    );
    assert!(
        !file_contains(&hvb_path, PASSWORD.as_bytes()),
        "the exported backup must never contain the password in clear"
    );

    backup
        .import_hvb_with_recovery_key(&hvb_path, &seed.recovery_phrase, &restored_path)
        .expect("import should succeed with the correct recovery phrase");

    // La DB restaurée doit se comporter comme l'originale : le mot de passe déverrouille le
    // compte, et le secret se déchiffre au contenu d'origine.
    let pool = open_pool(&restored_path).await;
    let user_repo = SqlxUserRepository::new(pool.clone());
    let auth = AuthServiceImpl::new(fast_crypto());
    let stored_envelope = user_repo
        .get_password_envelope_by_user_id(seed.user_id)
        .await
        .expect("query envelope")
        .expect("envelope present after restore");
    auth.upsert_password_envelope(USERNAME, stored_envelope)
        .await
        .expect("load credentials");
    let account_key = auth
        .derive_key_if_valid(USERNAME, secret(PASSWORD))
        .await
        .expect("derive key")
        .expect("the original password must still unlock the restored account");

    let vault_envelope = SqlxVaultRepository::new(pool.clone())
        .get_vault_key_envelope(seed.vault_id)
        .await
        .expect("query vault")
        .expect("vault envelope present");
    let vault_payload =
        deserialize_vault_key_envelope(&vault_envelope).expect("decode vault envelope");
    let vault_key = fast_crypto()
        .decrypt(&vault_payload, &account_key)
        .await
        .expect("decrypt vault key");

    let stored = SqlxSecretRepository::new(pool.clone())
        .list_by_vault_id(seed.vault_id)
        .await
        .expect("list secrets");
    let item = stored
        .first()
        .expect("restored vault should still hold its secret");
    let secret_payload =
        deserialize_vault_key_envelope(&item.secret_blob).expect("decode secret envelope");
    let decrypted = fast_crypto()
        .decrypt(&secret_payload, &vault_key)
        .await
        .expect("decrypt secret");

    assert_eq!(
        decrypted.expose_secret(),
        SECRET_PLAINTEXT,
        "the restored secret must match the original plaintext"
    );

    pool.close().await;
}

#[tokio::test]
async fn test_import_with_altered_backup_file_is_rejected() {
    let seed = seed().await;
    let backup = BackupServiceImpl::new();
    let hvb_path = seed.dir.path().join("export.hvb");
    let restored_path = seed.dir.path().join("restored.db");

    backup
        .export_hvb_with_recovery_key(&seed.db_path, &hvb_path, &seed.recovery_phrase)
        .expect("export should succeed");

    // Flip one bit in the middle of the file to simulate corruption/tampering.
    let mut bytes = std::fs::read(&hvb_path).expect("read exported backup");
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    std::fs::write(&hvb_path, &bytes).expect("write tampered backup");

    let result =
        backup.import_hvb_with_recovery_key(&hvb_path, &seed.recovery_phrase, &restored_path);

    assert!(
        matches!(
            result,
            Err(AppError::Recovery(
                RecoveryFailure::WrongPhraseOrAlteredFile
            ))
        ),
        "got {result:?}"
    );
    assert!(
        !restored_path.exists(),
        "a rejected import must not leave a partial database file"
    );
}

#[tokio::test]
async fn test_import_with_wrong_recovery_phrase_is_rejected() {
    let seed = seed().await;
    let backup = BackupServiceImpl::new();
    let hvb_path = seed.dir.path().join("export.hvb");
    let restored_path = seed.dir.path().join("restored.db");

    backup
        .export_hvb_with_recovery_key(&seed.db_path, &hvb_path, &seed.recovery_phrase)
        .expect("export should succeed");

    let wrong_phrase = backup
        .generate_recovery_key()
        .expect("generate a different phrase")
        .recovery_phrase;
    let result = backup.import_hvb_with_recovery_key(&hvb_path, &wrong_phrase, &restored_path);

    assert!(
        matches!(
            result,
            Err(AppError::Recovery(
                RecoveryFailure::WrongPhraseOrAlteredFile
            ))
        ),
        "got {result:?}"
    );
}

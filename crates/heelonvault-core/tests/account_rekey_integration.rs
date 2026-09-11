#![allow(clippy::disallowed_methods)]

//! Account key: bootstrap, legacy migration, password change and backup recovery. Every scenario
//! checks that each secret, the TOTP secret and the recovery phrase still open the account.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use heelonvault_core::errors::{AccessDeniedReason, AppError, RecoveryFailure};
use heelonvault_core::models::secret_item::{BlobStorage, SecretItem, SecretType};
use heelonvault_core::models::{UserRole, Vault};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::{
    SqlxUserRepository, UserKeyMaterialRepository, UserRepository,
};
use heelonvault_core::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_core::services::account_key::{ensure_recovery_key, open_with_recovery_phrase};
use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
use heelonvault_core::services::auth_service::{
    AuthService, AuthServiceImpl, PasswordEnvelope, decode_password_envelope,
};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{
    CryptoService, CryptoServiceImpl, KdfConfig, encode_envelope,
};
use heelonvault_core::services::recovery_service::reset_master_password_from_backup;
use heelonvault_core::services::user_service::{UserService, UserServiceImpl};
use heelonvault_core::services::vault_service::{
    VaultKeyEnvelopeRepository, deserialize_vault_key_envelope, serialize_vault_key_envelope,
};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use uuid::Uuid;

const USERNAME: &str = "alice";
const OLD_PASSWORD: &str = "the-password-alice-forgot-42";
const NEW_PASSWORD: &str = "a-brand-new-password-2026";
const TOTP_BASE32: &str = "JBSWY3DPEHPK3PXP";
const VAULT_KEY: [u8; 32] = [7; 32];

type TestUserService = UserServiceImpl<
    SqlxUserRepository,
    SqlxVaultRepository,
    SqlxVaultRepository,
    SqlxSecretRepository,
    AuthServiceImpl<CryptoServiceImpl>,
    CryptoServiceImpl,
>;

/// A single-account database; `key` is the key its vault keys were wrapped with.
struct Seed {
    dir: TempDir,
    db_path: PathBuf,
    user_id: Uuid,
    vault_id: Uuid,
    recovery_phrase: SecretString,
    secrets: Vec<(Uuid, Vec<u8>)>,
    key: Vec<u8>,
}

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

fn new_password() -> SecretString {
    SecretString::new(NEW_PASSWORD.into())
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
    serialize_vault_key_envelope(&payload)
}

async fn decrypt_with(key: &SecretBox<Vec<u8>>, blob: &SecretBox<Vec<u8>>) -> Option<Vec<u8>> {
    let payload = deserialize_vault_key_envelope(blob).ok()?;
    let plaintext = fast_crypto().decrypt(&payload, key).await.ok()?;
    Some(plaintext.expose_secret().clone())
}

/// A vault holding an inline and a file-backed secret, and a TOTP secret, all under `key`.
async fn populate(pool: &SqlitePool, user_id: Uuid, key: &[u8]) -> (Uuid, Vec<(Uuid, Vec<u8>)>) {
    let vault_id = Uuid::new_v4();
    SqlxVaultRepository::new(pool.clone())
        .create_vault_with_envelope(
            &Vault {
                id: vault_id,
                owner_user_id: user_id,
                name: "Perso".to_string(),
            },
            encrypt_with(key, &VAULT_KEY).await,
        )
        .await
        .expect("vault");

    let secret_repo = SqlxSecretRepository::new(pool.clone());
    let mut secrets = Vec::new();
    for (index, storage) in [BlobStorage::Inline, BlobStorage::File]
        .into_iter()
        .enumerate()
    {
        let plaintext = format!("secret-value-{index}").into_bytes();
        let item = SecretItem {
            id: Uuid::new_v4(),
            vault_id,
            secret_type: SecretType::Password,
            title: Some(format!("entry {index}")),
            metadata_json: None,
            tags: None,
            expires_at: None,
            created_at: None,
            modified_at: None,
            usage_count: 0,
            blob_storage: storage,
            secret_blob: SecretBox::new(Box::new(Vec::new())),
            deleted_at: None,
        };
        secret_repo
            .insert_secret_blob(&item, encrypt_with(&VAULT_KEY, &plaintext).await)
            .await
            .expect("secret");
        secrets.push((item.id, plaintext));
    }

    set_totp_secret(
        pool,
        user_id,
        &encrypt_with(key, TOTP_BASE32.as_bytes()).await,
    )
    .await;
    (vault_id, secrets)
}

async fn set_totp_secret(pool: &SqlitePool, user_id: Uuid, envelope: &SecretBox<Vec<u8>>) {
    sqlx::query("UPDATE users SET totp_secret = ?1 WHERE id = ?2")
        .bind(envelope.expose_secret().as_slice())
        .bind(user_id.to_string())
        .execute(pool)
        .await
        .expect("totp");
}

/// A vault created today: bootstrap seals a random account key with the password and the phrase.
async fn seed_account_key() -> Seed {
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
        secret(OLD_PASSWORD),
        phrase,
    )
    .await
    .expect("bootstrap");

    let key = boot.master_key.expose_secret().clone();
    let (vault_id, secrets) = populate(&pool, boot.user_id, &key).await;
    pool.close().await;
    Seed {
        dir,
        db_path,
        user_id: boot.user_id,
        vault_id,
        recovery_phrase: boot.recovery_phrase,
        secrets,
        key,
    }
}

/// A vault written before the account key: the password envelope holds the master key itself
/// and the recovery phrase is stored encrypted under it.
async fn seed_legacy() -> Seed {
    let dir = TempDir::new().expect("temp dir");
    let db_path = dir.path().join("source.db");
    let pool = open_pool(&db_path).await;

    let auth = AuthServiceImpl::new(fast_crypto());
    auth.create_user(USERNAME, secret(OLD_PASSWORD))
        .await
        .expect("legacy credentials");
    let master_key = auth
        .derive_key_if_valid(USERNAME, secret(OLD_PASSWORD))
        .await
        .expect("derive")
        .expect("master key");

    let user_id = Uuid::new_v4();
    let user_repo = SqlxUserRepository::new(pool.clone());
    user_repo
        .create_user_db(user_id, USERNAME, &UserRole::Admin)
        .await
        .expect("user");
    user_repo
        .update_password_envelope(
            user_id,
            auth.get_password_envelope(USERNAME)
                .await
                .expect("envelope"),
        )
        .await
        .expect("store envelope");

    let backup = BackupServiceImpl::new();
    let phrase = backup
        .generate_recovery_key()
        .expect("recovery phrase")
        .recovery_phrase;
    let phrase_payload = fast_crypto()
        .encrypt(
            &SecretBox::new(Box::new(phrase.expose_secret().as_bytes().to_vec())),
            &master_key,
        )
        .await
        .expect("encrypt phrase");
    user_repo
        .set_recovery_phrase_envelope(user_id, encode_envelope(&phrase_payload))
        .await
        .expect("phrase envelope");
    user_repo
        .set_recovery_verifier(
            user_id,
            backup.build_recovery_verifier(&phrase).expect("verifier"),
        )
        .await
        .expect("verifier");

    let key = master_key.expose_secret().clone();
    let (vault_id, secrets) = populate(&pool, user_id, &key).await;
    pool.close().await;
    Seed {
        dir,
        db_path,
        user_id,
        vault_id,
        recovery_phrase: phrase,
        secrets,
        key,
    }
}

async fn restored_copy(seed: &Seed) -> SqlitePool {
    let backup = BackupServiceImpl::new();
    let hvb_path = seed.dir.path().join("export.hvb");
    let restored_path = seed.dir.path().join("restored.db");
    backup
        .export_hvb_with_recovery_key(&seed.db_path, &hvb_path, &seed.recovery_phrase)
        .expect("export");
    backup
        .import_hvb_with_recovery_key(&hvb_path, &seed.recovery_phrase, &restored_path)
        .expect("import");
    open_pool(&restored_path).await
}

async fn stored_password_envelope(pool: &SqlitePool, user_id: Uuid) -> Vec<u8> {
    SqlxUserRepository::new(pool.clone())
        .get_password_envelope_by_user_id(user_id)
        .await
        .expect("query envelope")
        .expect("envelope")
        .expose_secret()
        .clone()
}

async fn stored_envelope(pool: &SqlitePool, user_id: Uuid) -> PasswordEnvelope {
    decode_password_envelope(&SecretBox::new(Box::new(
        stored_password_envelope(pool, user_id).await,
    )))
    .expect("decode envelope")
}

/// Mirrors startup: credentials are loaded from the stored envelope.
async fn logged_in_auth(
    pool: &SqlitePool,
    user_id: Uuid,
) -> Arc<AuthServiceImpl<CryptoServiceImpl>> {
    let auth = Arc::new(AuthServiceImpl::new(fast_crypto()));
    auth.upsert_password_envelope(
        USERNAME,
        SecretBox::new(Box::new(stored_password_envelope(pool, user_id).await)),
    )
    .await
    .expect("load credentials");
    auth
}

async fn login(pool: &SqlitePool, user_id: Uuid, password: &str) -> Option<SecretBox<Vec<u8>>> {
    logged_in_auth(pool, user_id)
        .await
        .derive_key_if_valid(USERNAME, secret(password))
        .await
        .expect("derive key")
}

fn user_service(
    pool: &SqlitePool,
    auth: Arc<AuthServiceImpl<CryptoServiceImpl>>,
) -> TestUserService {
    UserServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxSecretRepository::new(pool.clone()),
        auth,
        fast_crypto(),
    )
}

async fn add_second_account(pool: &SqlitePool) {
    SqlxUserRepository::new(pool.clone())
        .create_user_db(Uuid::new_v4(), "bob", &UserRole::User)
        .await
        .expect("second user");
}

fn file_contains(path: &Path, needle: &[u8]) -> bool {
    std::fs::read(path)
        .expect("read database file")
        .windows(needle.len())
        .any(|window| window == needle)
}

/// The vault, its secrets and the TOTP secret all open from `key`.
async fn assert_vault_opens(pool: &SqlitePool, seed: &Seed, key: &SecretBox<Vec<u8>>) {
    let vault_envelope = SqlxVaultRepository::new(pool.clone())
        .get_vault_key_envelope(seed.vault_id)
        .await
        .expect("query vault")
        .expect("vault envelope");
    let vault_key = decrypt_with(key, &vault_envelope)
        .await
        .expect("vault must open with the session key");
    assert_eq!(vault_key, VAULT_KEY.to_vec());

    let vault_key = SecretBox::new(Box::new(vault_key));
    let stored = SqlxSecretRepository::new(pool.clone())
        .list_by_vault_id(seed.vault_id)
        .await
        .expect("list secrets");
    assert_eq!(stored.len(), seed.secrets.len());
    for (secret_id, plaintext) in &seed.secrets {
        let item = stored
            .iter()
            .find(|item| item.id == *secret_id)
            .expect("secret present");
        assert_eq!(
            decrypt_with(&vault_key, &item.secret_blob).await.as_ref(),
            Some(plaintext),
            "secret {secret_id} must decrypt"
        );
    }

    let totp = SqlxUserRepository::new(pool.clone())
        .get_totp_secret(seed.user_id)
        .await
        .expect("query totp")
        .expect("2FA stays enabled");
    assert_eq!(
        decrypt_with(key, &totp).await,
        Some(TOTP_BASE32.as_bytes().to_vec()),
        "2FA login must keep working"
    );
}

/// Everything opens from `key`, and the recovery phrase alone unwraps `key`.
async fn assert_account_readable(pool: &SqlitePool, seed: &Seed, key: &SecretBox<Vec<u8>>) {
    assert_vault_opens(pool, seed, key).await;

    let user_repo = SqlxUserRepository::new(pool.clone());
    let sealed = user_repo
        .get_recovery_key_envelope(seed.user_id)
        .await
        .expect("query recovery key")
        .expect("recovery key envelope");
    let recovered = open_with_recovery_phrase(&fast_crypto(), &seed.recovery_phrase, &sealed)
        .await
        .expect("the recovery phrase must open the account key");
    assert_eq!(recovered.expose_secret(), key.expose_secret());
    assert!(
        user_repo
            .get_recovery_phrase_envelope(seed.user_id)
            .await
            .expect("query phrase")
            .is_none(),
        "the phrase itself is no longer stored"
    );
}

#[tokio::test]
async fn bootstrap_keeps_no_usable_key_in_the_database() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;

    assert!(stored_envelope(&pool, seed.user_id).await.is_account_key());
    let account_key = login(&pool, seed.user_id, OLD_PASSWORD)
        .await
        .expect("the password opens the account");
    assert_eq!(account_key.expose_secret(), &seed.key);
    assert_account_readable(&pool, &seed, &account_key).await;

    pool.close().await;
    assert!(
        !file_contains(&seed.db_path, &seed.key),
        "the account key must never be stored in clear"
    );
}

#[tokio::test]
async fn password_change_rewraps_only_the_account_key() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;
    let auth = logged_in_auth(&pool, seed.user_id).await;
    let service = user_service(&pool, Arc::clone(&auth));

    let session_key = service
        .change_master_password(seed.user_id, secret(OLD_PASSWORD), secret(NEW_PASSWORD))
        .await
        .expect("password change");

    assert_eq!(
        session_key.expose_secret(),
        &seed.key,
        "the account key survives a password change"
    );
    assert!(
        auth.derive_key_if_valid(USERNAME, secret(OLD_PASSWORD))
            .await
            .expect("derive")
            .is_none()
    );
    assert!(login(&pool, seed.user_id, OLD_PASSWORD).await.is_none());
    let after_restart = login(&pool, seed.user_id, NEW_PASSWORD)
        .await
        .expect("the new password logs in after a restart");
    assert_eq!(after_restart.expose_secret(), &seed.key);
    assert_account_readable(&pool, &seed, &session_key).await;
}

#[tokio::test]
async fn password_change_with_wrong_current_password_changes_nothing() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;
    let service = user_service(&pool, logged_in_auth(&pool, seed.user_id).await);
    let before = stored_password_envelope(&pool, seed.user_id).await;

    let result = service
        .change_master_password(
            seed.user_id,
            secret("definitely-not-the-password"),
            secret(NEW_PASSWORD),
        )
        .await;

    assert!(
        matches!(
            result,
            Err(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials
            ))
        ),
        "got {result:?}"
    );
    assert_eq!(stored_password_envelope(&pool, seed.user_id).await, before);
}

#[tokio::test]
async fn login_migrates_a_single_legacy_account() {
    let seed = seed_legacy().await;
    let pool = open_pool(&seed.db_path).await;
    let auth = logged_in_auth(&pool, seed.user_id).await;
    let service = user_service(&pool, Arc::clone(&auth));

    let account_key = service
        .upgrade_legacy_credentials(USERNAME, secret(OLD_PASSWORD))
        .await
        .expect("upgrade")
        .expect("a single legacy account is migrated");

    assert_ne!(account_key.expose_secret(), &seed.key);
    assert!(stored_envelope(&pool, seed.user_id).await.is_account_key());
    let in_memory = auth
        .derive_key_if_valid(USERNAME, secret(OLD_PASSWORD))
        .await
        .expect("derive")
        .expect("the same password still logs in");
    assert_eq!(in_memory.expose_secret(), account_key.expose_secret());
    let after_restart = login(&pool, seed.user_id, OLD_PASSWORD)
        .await
        .expect("login after a restart");
    assert_eq!(after_restart.expose_secret(), account_key.expose_secret());
    assert_account_readable(&pool, &seed, &account_key).await;
    assert!(
        !service
            .account_needs_recovery_key(seed.user_id)
            .await
            .expect("status")
    );
    assert!(
        service
            .upgrade_legacy_credentials(USERNAME, secret(OLD_PASSWORD))
            .await
            .expect("second login")
            .is_none()
    );

    pool.close().await;
    assert!(
        !file_contains(&seed.db_path, &seed.key),
        "the legacy master key must not survive the migration"
    );
}

#[tokio::test]
async fn login_leaves_multi_account_vaults_on_the_legacy_format() {
    let seed = seed_legacy().await;
    let pool = open_pool(&seed.db_path).await;
    add_second_account(&pool).await;
    let service = user_service(&pool, logged_in_auth(&pool, seed.user_id).await);

    let upgraded = service
        .upgrade_legacy_credentials(USERNAME, secret(OLD_PASSWORD))
        .await
        .expect("upgrade");

    assert!(upgraded.is_none());
    assert!(!stored_envelope(&pool, seed.user_id).await.is_account_key());
}

#[tokio::test]
async fn password_change_in_a_multi_account_vault_keeps_the_legacy_format() {
    let seed = seed_legacy().await;
    let pool = open_pool(&seed.db_path).await;
    add_second_account(&pool).await;
    let service = user_service(&pool, logged_in_auth(&pool, seed.user_id).await);

    let session_key = service
        .change_master_password(seed.user_id, secret(OLD_PASSWORD), secret(NEW_PASSWORD))
        .await
        .expect("password change");

    assert!(!stored_envelope(&pool, seed.user_id).await.is_account_key());
    let after_restart = login(&pool, seed.user_id, NEW_PASSWORD)
        .await
        .expect("the new password logs in");
    assert_eq!(after_restart.expose_secret(), session_key.expose_secret());
    assert_vault_opens(&pool, &seed, &session_key).await;
}

#[tokio::test]
async fn no_second_account_once_the_vault_uses_an_account_key() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;
    let auth = logged_in_auth(&pool, seed.user_id).await;

    let result = auth
        .create_user("bob", secret("another-password-2026"))
        .await;

    assert!(
        matches!(result, Err(AppError::SingleAccountVault)),
        "got {result:?}"
    );
}

#[tokio::test]
async fn account_key_backup_restores_with_its_phrase() {
    let seed = seed_account_key().await;
    let pool = restored_copy(&seed).await;

    let report = reset_master_password_from_backup(
        &pool,
        &fast_crypto(),
        &seed.recovery_phrase,
        &new_password(),
    )
    .await
    .expect("reset");

    assert_eq!(report.secrets_readable, 2);
    assert_eq!(report.secrets_unreadable, 0);
    assert!(report.recovery_key_available);
    assert!(login(&pool, seed.user_id, OLD_PASSWORD).await.is_none());
    let account_key = login(&pool, seed.user_id, NEW_PASSWORD)
        .await
        .expect("the new password logs in");
    assert_eq!(account_key.expose_secret(), &seed.key);
    assert_account_readable(&pool, &seed, &account_key).await;
}

#[tokio::test]
async fn legacy_backup_is_migrated_when_restored() {
    let seed = seed_legacy().await;
    let pool = restored_copy(&seed).await;

    let report = reset_master_password_from_backup(
        &pool,
        &fast_crypto(),
        &seed.recovery_phrase,
        &new_password(),
    )
    .await
    .expect("reset");

    assert_eq!(report.owner_vaults_rewrapped, 1);
    assert_eq!(report.secrets_readable, 2);
    assert!(report.recovery_key_available);
    assert!(stored_envelope(&pool, seed.user_id).await.is_account_key());
    let account_key = login(&pool, seed.user_id, NEW_PASSWORD)
        .await
        .expect("the new password logs in");
    assert_account_readable(&pool, &seed, &account_key).await;
}

#[tokio::test]
async fn reset_refuses_several_accounts_and_writes_nothing() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;
    add_second_account(&pool).await;
    let before = stored_password_envelope(&pool, seed.user_id).await;

    let result = reset_master_password_from_backup(
        &pool,
        &fast_crypto(),
        &seed.recovery_phrase,
        &new_password(),
    )
    .await;

    assert!(
        matches!(
            result,
            Err(AppError::Recovery(RecoveryFailure::MultipleAccounts))
        ),
        "got {result:?}"
    );
    assert_eq!(stored_password_envelope(&pool, seed.user_id).await, before);
}

#[tokio::test]
async fn reset_with_another_phrase_is_rejected() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;
    let before = stored_password_envelope(&pool, seed.user_id).await;
    let other_phrase = BackupServiceImpl::new()
        .generate_recovery_key()
        .expect("phrase")
        .recovery_phrase;

    let result =
        reset_master_password_from_backup(&pool, &fast_crypto(), &other_phrase, &new_password())
            .await;

    assert!(
        matches!(
            result,
            Err(AppError::Recovery(RecoveryFailure::InconsistentKeyMaterial))
        ),
        "got {result:?}"
    );
    assert_eq!(stored_password_envelope(&pool, seed.user_id).await, before);
}

#[tokio::test]
async fn account_key_backup_without_recovery_key_cannot_be_reset() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;
    sqlx::query("UPDATE users SET recovery_key_envelope = NULL WHERE id = ?1")
        .bind(seed.user_id.to_string())
        .execute(&pool)
        .await
        .expect("drop recovery key");

    let result = reset_master_password_from_backup(
        &pool,
        &fast_crypto(),
        &seed.recovery_phrase,
        &new_password(),
    )
    .await;

    assert!(
        matches!(
            result,
            Err(AppError::Recovery(RecoveryFailure::RecoveryKeyMissing))
        ),
        "got {result:?}"
    );
}

#[tokio::test]
async fn vault_not_opening_with_the_legacy_key_aborts_the_migration() {
    let seed = seed_legacy().await;
    let pool = open_pool(&seed.db_path).await;
    SqlxVaultRepository::new(pool.clone())
        .create_vault_with_envelope(
            &Vault {
                id: Uuid::new_v4(),
                owner_user_id: seed.user_id,
                name: "Corrupted".to_string(),
            },
            encrypt_with(&[3; 32], &VAULT_KEY).await,
        )
        .await
        .expect("foreign vault");
    let before = stored_password_envelope(&pool, seed.user_id).await;
    let service = user_service(&pool, logged_in_auth(&pool, seed.user_id).await);

    let result = service
        .upgrade_legacy_credentials(USERNAME, secret(OLD_PASSWORD))
        .await;

    assert!(
        matches!(
            result,
            Err(AppError::Recovery(RecoveryFailure::InconsistentKeyMaterial))
        ),
        "got {result:?}"
    );
    assert_eq!(stored_password_envelope(&pool, seed.user_id).await, before);
}

#[tokio::test]
async fn stale_totp_secret_is_cleared_during_migration() {
    let seed = seed_legacy().await;
    let pool = open_pool(&seed.db_path).await;
    set_totp_secret(
        &pool,
        seed.user_id,
        &encrypt_with(&[9; 32], TOTP_BASE32.as_bytes()).await,
    )
    .await;
    let service = user_service(&pool, logged_in_auth(&pool, seed.user_id).await);

    service
        .upgrade_legacy_credentials(USERNAME, secret(OLD_PASSWORD))
        .await
        .expect("upgrade")
        .expect("migrated");

    let totp = SqlxUserRepository::new(pool.clone())
        .get_totp_secret(seed.user_id)
        .await
        .expect("query totp");
    assert!(
        totp.is_none(),
        "an unusable TOTP secret must not lock the user out"
    );
}

#[tokio::test]
async fn migration_without_a_readable_phrase_is_fixed_by_the_next_export() {
    let seed = seed_legacy().await;
    let pool = open_pool(&seed.db_path).await;
    let user_repo = SqlxUserRepository::new(pool.clone());
    let stale = fast_crypto()
        .encrypt(
            &SecretBox::new(Box::new(b"not the phrase".to_vec())),
            &SecretBox::new(Box::new(vec![5; 32])),
        )
        .await
        .expect("encrypt");
    user_repo
        .set_recovery_phrase_envelope(seed.user_id, encode_envelope(&stale))
        .await
        .expect("stale phrase envelope");
    let service = user_service(&pool, logged_in_auth(&pool, seed.user_id).await);

    let account_key = service
        .upgrade_legacy_credentials(USERNAME, secret(OLD_PASSWORD))
        .await
        .expect("upgrade")
        .expect("migrated");
    assert!(
        service
            .account_needs_recovery_key(seed.user_id)
            .await
            .expect("status")
    );

    assert!(
        ensure_recovery_key(
            &user_repo,
            &fast_crypto(),
            seed.user_id,
            &account_key,
            &seed.recovery_phrase
        )
        .await
        .expect("seal")
    );
    assert!(
        !ensure_recovery_key(
            &user_repo,
            &fast_crypto(),
            seed.user_id,
            &account_key,
            &seed.recovery_phrase
        )
        .await
        .expect("already sealed")
    );
    assert!(
        !service
            .account_needs_recovery_key(seed.user_id)
            .await
            .expect("status")
    );
    assert_account_readable(&pool, &seed, &account_key).await;
}

#[tokio::test]
async fn vault_without_key_envelope_is_skipped() {
    let seed = seed_account_key().await;
    let pool = open_pool(&seed.db_path).await;
    SqlxVaultRepository::new(pool.clone())
        .create_vault(&Vault {
            id: Uuid::new_v4(),
            owner_user_id: seed.user_id,
            name: "Orphan".to_string(),
        })
        .await
        .expect("orphan vault");

    let report = reset_master_password_from_backup(
        &pool,
        &fast_crypto(),
        &seed.recovery_phrase,
        &new_password(),
    )
    .await
    .expect("reset");

    assert_eq!(report.vaults_without_key, 1);
    assert!(login(&pool, seed.user_id, NEW_PASSWORD).await.is_some());
}

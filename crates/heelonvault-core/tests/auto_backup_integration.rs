// `#[tokio::test]` expands to `Runtime::build().expect(..)`, which `disallowed_methods`
// reports here. Test code stays covered by `clippy::unwrap_used` / `clippy::expect_used`,
// enforced by `cargo clippy-secure` in CI.
#![allow(clippy::disallowed_methods)]

//! Automatic backups on a real account (bootstrap, migrated schema, synthetic data): sealing,
//! restore with the master password and the recovery phrase, mirrors, retention, health and
//! change detection.

use anyhow::Context as _;
use std::path::{Path, PathBuf};

use heelonvault_core::errors::{AccessDeniedReason, AppError, RecoveryFailure};
use heelonvault_core::models::secret_item::{BlobStorage, SecretItem, SecretType};
use heelonvault_core::models::{User, UserRole, Vault};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
use heelonvault_core::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_core::services::auto_backup::{
    self, AccountEnvelopes, AutoBackupSettings, BackupCredential, BackupJournal, BackupRequest,
    BackupTrigger, MirrorStatus, RunOutcome, container, health, store,
};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{CryptoServiceImpl, KdfConfig};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const PASSWORD: &str = "a-synthetic-master-password-2026";

fn fast_crypto() -> CryptoServiceImpl {
    CryptoServiceImpl::new(KdfConfig {
        memory_cost_kib: 1024,
        time_cost: 1,
        parallelism: 1,
        output_len: 32,
    })
}

struct Account {
    dir: TempDir,
    db_path: PathBuf,
    pool: SqlitePool,
    user: User,
    account_key: SecretBox<Vec<u8>>,
    recovery_phrase: SecretString,
    vault_id: Uuid,
}

impl Account {
    fn backups(&self) -> PathBuf {
        self.dir.path().join("backups")
    }

    async fn envelopes(&self) -> anyhow::Result<AccountEnvelopes> {
        auto_backup::load_envelopes(&SqlxUserRepository::new(self.pool.clone()), self.user.id)
            .await
            .context("envelopes")
    }

    async fn run(
        &self,
        settings: &AutoBackupSettings,
        trigger: BackupTrigger,
    ) -> anyhow::Result<auto_backup::BackupRunRecord> {
        let backups = self.backups();
        auto_backup::run_backup(
            &self.pool,
            &self.user,
            &self.account_key,
            &self.envelopes().await?,
            BackupRequest::new(&self.db_path, &backups, settings, trigger),
        )
        .await
        .context("run backup")
    }
}

async fn open_pool(path: &Path) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .context("open sqlite pool")?;
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../heelonvault-app/migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .context("load migrations")?
        .run(&pool)
        .await
        .context("run migrations")?;
    Ok(pool)
}

async fn seed() -> anyhow::Result<Account> {
    let dir = TempDir::new().context("temp dir")?;
    let db_path = dir.path().join("vault.db");
    let pool = open_pool(&db_path).await?;
    let backup = BackupServiceImpl::new();
    let phrase = backup
        .generate_recovery_key()
        .context("phrase")?
        .recovery_phrase;
    let users = SqlxUserRepository::new(pool.clone());
    let boot = bootstrap_first_admin_with_recovery(
        &users,
        &AuthServiceImpl::new(fast_crypto()),
        &backup,
        &fast_crypto(),
        "alice",
        SecretBox::new(Box::new(PASSWORD.as_bytes().to_vec())),
        phrase,
    )
    .await
    .context("bootstrap")?;

    let vault_id = Uuid::new_v4();
    SqlxVaultRepository::new(pool.clone())
        .create_vault_with_envelope(
            &Vault {
                id: vault_id,
                owner_user_id: boot.user_id,
                name: "Perso".to_string(),
            },
            SecretBox::new(Box::new(vec![1; 60])),
        )
        .await
        .context("vault")?;
    insert_entry(&pool, vault_id).await?;
    let user = users
        .get_by_id(boot.user_id)
        .await
        .context("user")?
        .context("user exists")?;
    Ok(Account {
        dir,
        db_path,
        pool,
        user,
        account_key: boot.master_key,
        recovery_phrase: boot.recovery_phrase,
        vault_id,
    })
}

async fn insert_entry(pool: &SqlitePool, vault_id: Uuid) -> anyhow::Result<Uuid> {
    let item = SecretItem {
        id: Uuid::new_v4(),
        vault_id,
        secret_type: SecretType::Password,
        title: Some("Synthetic entry".to_string()),
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
        .insert_secret_blob(&item, SecretBox::new(Box::new(vec![2; 48])))
        .await
        .context("secret")?;
    Ok(item.id)
}

fn local_backup(
    account: &Account,
    record: &auto_backup::BackupRunRecord,
) -> anyhow::Result<Vec<u8>> {
    let name = record.file_name.as_ref().context("file name")?;
    std::fs::read(account.backups().join(name)).context("read backup")
}

#[tokio::test]
async fn a_backup_opens_with_the_password_or_the_phrase_and_reaches_ready_mirrors()
-> anyhow::Result<()> {
    let account = seed().await?;
    let nas = account.dir.path().join("nas-share");
    std::fs::create_dir(&nas).context("mirror")?;
    let unmounted = account.dir.path().join("beestation").join("HeelonVault");
    let mut settings = AutoBackupSettings::default();
    settings.mirrors = vec![nas.clone(), unmounted.clone()];

    let record = account.run(&settings, BackupTrigger::Manual).await?;
    assert_eq!(record.outcome, RunOutcome::Partial, "one mirror is missing");
    let health = record.health.clone().context("health")?;
    assert!(health.is_healthy());
    assert_eq!(
        (health.secret_count, health.vault_count, health.user_count),
        (1, 1, 1)
    );
    assert!(health.schema_version.is_some());
    assert_eq!(record.mirrors[0].status, MirrorStatus::Copied);
    assert_eq!(record.mirrors[1].status, MirrorStatus::Missing);
    assert!(!unmounted.exists(), "a missing mirror is never created");

    let bytes = local_backup(&account, &record)?;
    let name = record.file_name.clone().context("name")?;
    assert_eq!(
        std::fs::read(nas.join(&name)).context("mirror copy")?,
        bytes
    );
    assert_eq!(
        record.sha256.as_deref(),
        Some(store::sha256_hex(&bytes).as_str())
    );

    let info = container::read_info(&bytes)?;
    assert_eq!(info.user_id, account.user.id);
    assert!(info.opens_with_recovery_phrase);

    let crypto = fast_crypto();
    let by_password = container::open_with_credential(
        &bytes,
        &crypto,
        &BackupCredential::MasterPassword(SecretString::new(PASSWORD.into())),
    )
    .await?;
    let by_phrase = container::open_with_credential(
        &bytes,
        &crypto,
        &BackupCredential::RecoveryPhrase(account.recovery_phrase.clone()),
    )
    .await?;
    assert!(by_password.starts_with(b"SQLite format 3\0"));
    assert_eq!(by_password.as_slice(), by_phrase.as_slice());

    let wrong = container::open_with_credential(
        &bytes,
        &crypto,
        &BackupCredential::MasterPassword(SecretString::new("not-the-password".into())),
    )
    .await;
    assert!(matches!(
        wrong,
        Err(AppError::Recovery(
            RecoveryFailure::WrongPhraseOrAlteredFile
        ))
    ));

    // Restore test: decrypted into a temporary copy, checked, then deleted.
    let work = account.dir.path().join("work");
    let tested =
        health::test_restore(&account.backups().join(&name), &account.account_key, &work).await?;
    assert!(tested.is_healthy());
    assert_eq!(tested.secret_count, 1);
    assert_eq!(std::fs::read_dir(&work).context("work dir")?.count(), 0);

    // Restore path of the login screen: phrase-unlocked copy written to a staging file.
    assert!(container::is_auto_backup(&bytes));
    let staging = account.dir.path().join("staging").join("restored.db");
    let restored = auto_backup::restore_to_file(
        &account.backups().join(&name),
        &crypto,
        &BackupCredential::RecoveryPhrase(account.recovery_phrase.clone()),
        &staging,
    )
    .await?;
    assert_eq!(restored.user_id, account.user.id);
    assert_eq!(
        std::fs::read(&staging).context("staging")?,
        by_phrase.as_slice()
    );

    let journal = BackupJournal::load(&account.backups());
    assert_eq!(journal.runs.len(), 1);
    assert_eq!(journal.last_success(), Some(&record));
    Ok(())
}

#[tokio::test]
async fn an_altered_backup_is_refused() -> anyhow::Result<()> {
    let account = seed().await?;
    let record = account
        .run(&AutoBackupSettings::default(), BackupTrigger::Manual)
        .await?;
    let mut bytes = local_backup(&account, &record)?;
    // Inside the JSON header: authenticated as AAD.
    let header_byte = bytes
        .iter()
        .position(|byte| *byte == b'"')
        .context("header")?;
    bytes[header_byte + 1] ^= 0x01;
    assert!(container::open_with_account_key(&bytes, &account.account_key).is_err());

    let mut tail = local_backup(&account, &record)?;
    if let Some(last) = tail.last_mut() {
        *last ^= 0x01;
    }
    assert!(matches!(
        container::open_with_account_key(&tail, &account.account_key),
        Err(AppError::Recovery(
            RecoveryFailure::WrongPhraseOrAlteredFile
        ))
    ));
    Ok(())
}

#[tokio::test]
async fn close_time_backups_follow_content_changes() -> anyhow::Result<()> {
    let account = seed().await?;
    let settings = AutoBackupSettings::default();
    let now = OffsetDateTime::now_utc();
    let empty = BackupJournal::default();
    let fingerprint = auto_backup::content_fingerprint(&account.pool).await?;
    assert!(auto_backup::is_due(
        &settings,
        &empty,
        BackupTrigger::Unlock,
        now,
        None
    ));
    assert!(auto_backup::is_due(
        &settings,
        &empty,
        BackupTrigger::Close,
        now,
        Some(&fingerprint)
    ));

    account.run(&settings, BackupTrigger::Manual).await?;
    let journal = BackupJournal::load(&account.backups());
    assert!(!auto_backup::is_due(
        &settings,
        &journal,
        BackupTrigger::Close,
        now,
        Some(&fingerprint)
    ));
    assert!(!auto_backup::is_due(
        &settings,
        &journal,
        BackupTrigger::Unlock,
        now + Duration::hours(23),
        None
    ));
    assert!(auto_backup::is_due(
        &settings,
        &journal,
        BackupTrigger::Unlock,
        now + Duration::hours(25),
        None
    ));

    // Logging in again is not a change; copying or editing an entry is.
    sqlx::query("INSERT INTO login_history (user_id, login_at) VALUES (?1, ?2)")
        .bind(account.user.id.to_string())
        .bind("2026-11-23T08:00:00Z")
        .execute(&account.pool)
        .await
        .context("login history")?;
    assert_eq!(
        auto_backup::content_fingerprint(&account.pool).await?,
        fingerprint
    );
    sqlx::query("UPDATE secret_items SET usage_count = usage_count + 1")
        .execute(&account.pool)
        .await
        .context("copy counter")?;
    let changed = auto_backup::content_fingerprint(&account.pool).await?;
    assert_ne!(changed, fingerprint);
    assert!(auto_backup::is_due(
        &settings,
        &journal,
        BackupTrigger::Close,
        now,
        Some(&changed)
    ));

    let mut off = AutoBackupSettings::default();
    off.schedule = auto_backup::BackupSchedule::Off;
    assert!(!auto_backup::is_due(
        &off,
        &empty,
        BackupTrigger::Unlock,
        now,
        None
    ));
    assert!(auto_backup::is_due(
        &off,
        &empty,
        BackupTrigger::Manual,
        now,
        None
    ));
    Ok(())
}

#[tokio::test]
async fn a_damaged_database_is_not_backed_up_and_keeps_older_backups() -> anyhow::Result<()> {
    let account = seed().await?;
    let mut settings = AutoBackupSettings::default();
    settings.retention = auto_backup::RetentionPolicy::new(1, 0, 0);
    let first = account.run(&settings, BackupTrigger::Manual).await?;
    assert!(first.succeeded());

    // A dangling reference, written with foreign keys disabled.
    let mut conn = account.pool.acquire().await.context("connection")?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await
        .context("fk off")?;
    sqlx::query(
        "INSERT INTO secret_items (id, vault_id, secret_type, title, blob_storage, secret_blob) \
         VALUES (?1, ?2, 'password', 'orphan', 'inline', x'00')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(Uuid::new_v4().to_string())
    .execute(&mut *conn)
    .await
    .context("orphan")?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await
        .context("fk on")?;
    drop(conn);

    let second = account.run(&settings, BackupTrigger::Manual).await?;
    assert!(matches!(second.outcome, RunOutcome::Failed(_)));
    assert!(second.file_name.is_none());
    assert_eq!(
        second
            .health
            .as_ref()
            .map(|health| health.foreign_key_violations),
        Some(1)
    );
    let left = store::list_backups(&account.backups())?;
    assert_eq!(left.len(), 1, "the previous backup is kept");
    assert_eq!(
        Some(left[0].path.clone()),
        first.file_name.map(|name| account.backups().join(name))
    );
    let _ = account.vault_id;
    Ok(())
}

#[tokio::test]
async fn only_an_admin_with_an_account_key_can_back_up() -> anyhow::Result<()> {
    let account = seed().await?;
    let mut member = account.user.clone();
    member.role = UserRole::User;
    let backups = account.backups();
    let settings = AutoBackupSettings::default();
    let denied = auto_backup::run_backup(
        &account.pool,
        &member,
        &account.account_key,
        &account.envelopes().await?,
        BackupRequest::new(&account.db_path, &backups, &settings, BackupTrigger::Manual),
    )
    .await;
    assert!(matches!(
        denied,
        Err(AppError::Authorization(AccessDeniedReason::Unauthorized))
            | Err(AppError::Authorization(_))
    ));
    assert!(!backups.exists());

    // A legacy envelope holds the derived key itself: it must never be copied into a backup.
    let auth = AuthServiceImpl::new(fast_crypto());
    auth.create_user(
        "legacy",
        SecretBox::new(Box::new(PASSWORD.as_bytes().to_vec())),
    )
    .await
    .context("legacy user")?;
    let legacy = AccountEnvelopes::new(
        account.user.id,
        auth.get_password_envelope("legacy")
            .await
            .context("legacy envelope")?,
        None,
    );
    let snapshot = std::fs::read(&account.db_path).context("db bytes")?;
    assert!(
        container::seal(
            &snapshot,
            &account.account_key,
            &legacy,
            OffsetDateTime::now_utc()
        )
        .is_err()
    );
    assert!(account.account_key.expose_secret().len() == 32);
    Ok(())
}

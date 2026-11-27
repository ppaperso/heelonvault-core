//! Automatic backups: sealed with the account key, written and verified locally, copied to
//! optional mirror folders (e.g. a NAS share), pruned, and recorded with a database health
//! check.
//!
//! Restoring needs the master password or the recovery phrase (see [`container`]). The manual
//! `.hvb` export, sealed with the recovery phrase, remains the portable format.

pub mod container;
pub mod health;
pub mod journal;
pub mod retention;
pub mod store;

use std::path::{Path, PathBuf};

use secrecy::SecretBox;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use time::{Duration, OffsetDateTime};
use tracing::{info, warn};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::errors::AppError;
use crate::models::User;
use crate::repositories::user_repository::{UserKeyMaterialRepository, UserRepository};
use crate::services::access_control::{Action, Resource, check_permission};
use crate::services::backup_application_service::snapshot_database;

pub use container::{AccountEnvelopes, BackupCredential, BackupInfo};
pub use health::DatabaseHealth;
pub use journal::{
    BackupJournal, BackupRunRecord, BackupTrigger, MirrorOutcome, MirrorStatus, RunOutcome,
};
pub use retention::RetentionPolicy;
pub use store::{BackupFile, DestinationState};

/// Interval after which the first unlock takes a new backup.
pub const DAILY_INTERVAL: Duration = Duration::hours(24);

/// When automatic backups run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BackupSchedule {
    Off,
    /// At unlock, when the last successful backup is older than [`DAILY_INTERVAL`].
    Daily,
    /// When closing or locking the session, if the content changed since the last backup.
    OnClose,
    #[default]
    DailyAndOnClose,
}

impl BackupSchedule {
    pub fn on_unlock(self) -> bool {
        matches!(self, Self::Daily | Self::DailyAndOnClose)
    }

    pub fn on_close(self) -> bool {
        matches!(self, Self::OnClose | Self::DailyAndOnClose)
    }
}

/// User settings, persisted by the application.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AutoBackupSettings {
    #[serde(default)]
    pub schedule: BackupSchedule,
    /// Existing folders that receive a copy of each backup (never created by the app).
    #[serde(default)]
    pub mirrors: Vec<PathBuf>,
    #[serde(default)]
    pub retention: RetentionPolicy,
}

/// Whether `trigger` should start a backup now.
///
/// `fingerprint` is the current [`content_fingerprint`]; it is only needed for
/// [`BackupTrigger::Close`].
pub fn is_due(
    settings: &AutoBackupSettings,
    journal: &BackupJournal,
    trigger: BackupTrigger,
    now: OffsetDateTime,
    fingerprint: Option<&str>,
) -> bool {
    let last = journal.last_success();
    match trigger {
        BackupTrigger::Unlock => {
            settings.schedule.on_unlock()
                && last.is_none_or(|run| now - run.finished_at >= DAILY_INTERVAL)
        }
        BackupTrigger::Close => {
            settings.schedule.on_close()
                && last.is_none_or(|run| run.fingerprint.as_deref() != fingerprint)
        }
        BackupTrigger::Manual | BackupTrigger::PasswordChange => true,
    }
}

async fn table_exists(pool: &SqlitePool, table: &str) -> Result<bool, AppError> {
    let found: Option<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .bind(table)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

/// Digest of what a backup preserves: entries (including copy counters and trash), vaults,
/// accounts and shares. Login history and audit rows are left out, so merely opening the
/// application does not count as a change. Computed in SQLite over ciphertexts and metadata;
/// only the digest leaves this function.
pub async fn content_fingerprint(pool: &SqlitePool) -> Result<String, AppError> {
    const SOURCES: [(&str, &str); 6] = [
        (
            "secret_items",
            "SELECT id || '|' || modified_at || '|' || usage_count || '|' || \
             coalesce(deleted_at, '') || '|' || coalesce(title, '') || '|' || \
             coalesce(metadata_json, '') || '|' || coalesce(tags, '') || '|' || \
             coalesce(expires_at, '') || '|' || coalesce(hex(secret_blob), '') || '|' || \
             coalesce(hex(file_blob_ref), '') FROM secret_items ORDER BY id",
        ),
        (
            "vaults",
            "SELECT id || '|' || owner_user_id || '|' || name || '|' || \
             coalesce(hex(vault_key_envelope), '') FROM vaults ORDER BY id",
        ),
        (
            "users",
            "SELECT id || '|' || username || '|' || role || '|' || \
             coalesce(hex(password_envelope), '') FROM users ORDER BY id",
        ),
        (
            "vault_key_shares",
            "SELECT count(*) || '' FROM vault_key_shares",
        ),
        ("teams", "SELECT count(*) || '' FROM teams"),
        ("team_members", "SELECT count(*) || '' FROM team_members"),
    ];
    let mut hasher = Sha256::new();
    for (table, query) in SOURCES {
        hasher.update(table.as_bytes());
        hasher.update(b"\n");
        if !table_exists(pool, table).await? {
            continue;
        }
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
            .fetch_all(pool)
            .await?;
        for row in rows {
            let row = Zeroizing::new(row);
            hasher.update(row.as_bytes());
            hasher.update(b"\n");
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Reads the envelopes copied into each backup of `user_id`.
pub async fn load_envelopes<R>(users: &R, user_id: Uuid) -> Result<AccountEnvelopes, AppError>
where
    R: UserRepository + UserKeyMaterialRepository + Sync,
{
    let password_envelope = users
        .get_password_envelope_by_user_id(user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("password envelope not found".to_string()))?;
    let recovery_envelope = users.get_recovery_key_envelope(user_id).await?;
    Ok(AccountEnvelopes::new(
        user_id,
        password_envelope,
        recovery_envelope,
    ))
}

/// Where and why a backup runs.
#[non_exhaustive]
pub struct BackupRequest<'a> {
    pub db_path: &'a Path,
    /// Local backup folder, created when missing; also holds the journal.
    pub backup_dir: &'a Path,
    pub settings: &'a AutoBackupSettings,
    pub trigger: BackupTrigger,
}

impl<'a> BackupRequest<'a> {
    pub fn new(
        db_path: &'a Path,
        backup_dir: &'a Path,
        settings: &'a AutoBackupSettings,
        trigger: BackupTrigger,
    ) -> Self {
        Self {
            db_path,
            backup_dir,
            settings,
            trigger,
        }
    }
}

fn failure(
    started_at: OffsetDateTime,
    trigger: BackupTrigger,
    reason: &str,
    health: Option<DatabaseHealth>,
) -> BackupRunRecord {
    BackupRunRecord {
        started_at,
        finished_at: OffsetDateTime::now_utc(),
        trigger,
        outcome: RunOutcome::Failed(reason.to_string()),
        file_name: None,
        size: 0,
        sha256: None,
        fingerprint: None,
        health,
        mirrors: Vec::new(),
        pruned: 0,
    }
}

/// Copies the local backup into each mirror and prunes the mirrors that received it.
/// Blocking: run through `spawn_blocking`.
fn copy_to_mirrors(
    mirrors: &[PathBuf],
    file_name: &str,
    bytes: &[u8],
    retention: &RetentionPolicy,
) -> Vec<MirrorOutcome> {
    mirrors
        .iter()
        .map(|mirror| {
            let status = match store::check_mirror(mirror) {
                DestinationState::Ready => match store::write_verified(mirror, file_name, bytes) {
                    Ok(_) => MirrorStatus::Copied,
                    Err(err) => MirrorStatus::Failed(err.to_string()),
                },
                DestinationState::Missing => MirrorStatus::Missing,
                DestinationState::NotADirectory => MirrorStatus::NotADirectory,
                DestinationState::Unreachable(message) => MirrorStatus::Failed(message),
            };
            // Prune only a mirror that holds the new backup.
            let pruned = if status == MirrorStatus::Copied {
                match store::prune(mirror, retention) {
                    Ok(removed) => u32::try_from(removed.len()).unwrap_or(u32::MAX),
                    Err(err) => {
                        warn!(mirror = %mirror.display(), %err, "mirror pruning failed");
                        0
                    }
                }
            } else {
                0
            };
            MirrorOutcome {
                path: mirror.clone(),
                status,
                pruned,
            }
        })
        .collect()
}

/// Runs one automatic backup and records it in the journal.
///
/// Only an authorization failure is returned as an error; any other failure is part of the
/// returned record (and of the journal). A database that fails its integrity check is not
/// backed up and nothing is pruned, so a damaged database can never push good backups out.
pub async fn run_backup(
    pool: &SqlitePool,
    actor: &User,
    account_key: &SecretBox<Vec<u8>>,
    envelopes: &AccountEnvelopes,
    request: BackupRequest<'_>,
) -> Result<BackupRunRecord, AppError> {
    check_permission(actor, Action::BackupExport, &Resource::Global)?;
    if envelopes.user_id != actor.id {
        return Err(AppError::Validation(
            "backup envelopes do not belong to the actor".to_string(),
        ));
    }
    let started_at = OffsetDateTime::now_utc();
    let record = match backup_steps(pool, account_key, envelopes, &request, started_at).await {
        Ok(record) => record,
        Err((reason, health)) => {
            warn!(trigger = ?request.trigger, %reason, "automatic backup failed");
            failure(started_at, request.trigger, &reason, health)
        }
    };
    if let Err(err) = BackupJournal::append(request.backup_dir, record.clone()) {
        warn!(%err, "backup journal could not be written");
    }
    Ok(record)
}

type StepError = (String, Option<DatabaseHealth>);

async fn backup_steps(
    pool: &SqlitePool,
    account_key: &SecretBox<Vec<u8>>,
    envelopes: &AccountEnvelopes,
    request: &BackupRequest<'_>,
    started_at: OffsetDateTime,
) -> Result<BackupRunRecord, StepError> {
    let fail = |health: Option<&DatabaseHealth>| {
        let health = health.cloned();
        move |err: AppError| (err.to_string(), health)
    };
    let health = health::check_database(pool, request.db_path)
        .await
        .map_err(fail(None))?;
    if !health.is_healthy() {
        return Err((
            "database integrity check failed; previous backups were kept".to_string(),
            Some(health),
        ));
    }
    let fingerprint = content_fingerprint(pool)
        .await
        .map_err(fail(Some(&health)))?;

    let snapshot = snapshot_database(pool, request.db_path, "auto-snapshot")
        .await
        .map_err(fail(Some(&health)))?;
    let plaintext = Zeroizing::new(
        std::fs::read(&snapshot.path)
            .map_err(AppError::Io)
            .map_err(fail(Some(&health)))?,
    );
    drop(snapshot);
    let sealed = container::seal(&plaintext, account_key, envelopes, started_at)
        .map_err(fail(Some(&health)))?;
    // The account key must open what was written before anything is pruned.
    let reopened =
        container::open_with_account_key(&sealed, account_key).map_err(fail(Some(&health)))?;
    if reopened.as_slice() != plaintext.as_slice() {
        return Err(("backup self-check failed".to_string(), Some(health)));
    }
    drop(reopened);
    drop(plaintext);

    let file_name = store::file_name_for(started_at);
    store::ensure_local_dir(request.backup_dir).map_err(fail(Some(&health)))?;
    let sha256 = store::write_verified(request.backup_dir, &file_name, &sealed)
        .map_err(fail(Some(&health)))?;
    let pruned = match store::prune(request.backup_dir, &request.settings.retention) {
        Ok(removed) => u32::try_from(removed.len()).unwrap_or(u32::MAX),
        Err(err) => {
            warn!(%err, "local backup pruning failed");
            0
        }
    };

    let size = sealed.len() as u64;
    let mirrors = if request.settings.mirrors.is_empty() {
        Vec::new()
    } else {
        let targets = request.settings.mirrors.clone();
        let retention = request.settings.retention;
        let name = file_name.clone();
        tokio::task::spawn_blocking(move || copy_to_mirrors(&targets, &name, &sealed, &retention))
            .await
            .map_err(|_| {
                (
                    "mirror copy was interrupted".to_string(),
                    Some(health.clone()),
                )
            })?
    };
    let outcome = if mirrors
        .iter()
        .all(|mirror| mirror.status == MirrorStatus::Copied)
    {
        RunOutcome::Success
    } else {
        RunOutcome::Partial
    };
    info!(
        trigger = ?request.trigger,
        file = %file_name,
        size,
        mirrors = mirrors.len(),
        ?outcome,
        "automatic backup completed"
    );
    Ok(BackupRunRecord {
        started_at,
        finished_at: OffsetDateTime::now_utc(),
        trigger: request.trigger,
        outcome,
        file_name: Some(file_name),
        size,
        sha256: Some(sha256),
        fingerprint: Some(fingerprint),
        health: Some(health),
        mirrors,
        pruned,
    })
}

/// Decrypts an automatic backup with the master password or the recovery phrase and writes
/// the database to `target` (owner-only), e.g. the staging file of a restore. Returns the
/// backup description.
pub async fn restore_to_file<C>(
    backup_path: &Path,
    crypto: &C,
    credential: &BackupCredential,
    target: &Path,
) -> Result<BackupInfo, AppError>
where
    C: crate::services::crypto_service::CryptoService + Send + Sync,
{
    let bytes = std::fs::read(backup_path).map_err(AppError::Io)?;
    let info = container::read_info(&bytes)?;
    let plaintext = container::open_with_credential(&bytes, crypto, credential).await?;
    if let Some(parent) = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        crate::utils::private_fs::create_private_dir_all(parent).map_err(AppError::Io)?;
    }
    crate::utils::private_fs::write_private(target, &plaintext).map_err(AppError::Io)?;
    Ok(info)
}

/// Latest backup in the local folder, for the restore test.
pub fn latest_local_backup(backup_dir: &Path) -> Result<Option<BackupFile>, AppError> {
    Ok(store::list_backups(backup_dir)?.into_iter().next())
}

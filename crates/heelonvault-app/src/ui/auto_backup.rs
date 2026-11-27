//! Automatic backups in the application: settings file, background runs and toasts.
//!
//! `install` is called once the database is open; until then (and in widget tests) every
//! request is ignored. Runs happen on a worker thread, one at a time, with a copy of the
//! session key that is wiped when the run ends. A run started at logout keeps going while
//! the login screen is shown; quitting waits for it a bounded time (`wait_for_pending`).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gtk4::glib;
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::services::auto_backup::{
    self, AutoBackupSettings, BackupJournal, BackupRequest, BackupRunRecord, BackupTrigger,
    RunOutcome,
};
use secrecy::SecretBox;
use sqlx::SqlitePool;
use time::OffsetDateTime;
use tracing::{info, warn};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::window_sizing::ui_state_file_path;

const SETTINGS_FILE: &str = "auto_backup.json";
/// Name of the local backup folder, next to the database.
const BACKUP_DIR_NAME: &str = "backups";

struct Context {
    runtime: Arc<tokio::runtime::Runtime>,
    pool: SqlitePool,
    db_path: PathBuf,
}

static CONTEXT: OnceLock<Context> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(false);
static PENDING: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

/// Enables automatic backups for the open database.
pub fn install(runtime: Arc<tokio::runtime::Runtime>, pool: SqlitePool, db_path: PathBuf) {
    if CONTEXT
        .set(Context {
            runtime,
            pool,
            db_path,
        })
        .is_err()
    {
        warn!("automatic backups were already installed");
    }
}

/// Local backup folder: `backups/` next to the database.
pub fn backup_dir_for(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(BACKUP_DIR_NAME)
}

/// Local backup folder of the installed database, if any.
pub fn backup_dir() -> Option<PathBuf> {
    CONTEXT
        .get()
        .map(|context| backup_dir_for(&context.db_path))
}

/// Settings; a missing or unreadable file gives the defaults (daily and on close, local only).
pub fn load_settings() -> AutoBackupSettings {
    // Tests never read the user's settings (they may name a real NAS folder).
    if cfg!(test) {
        return AutoBackupSettings::default();
    }
    fs::read_to_string(ui_state_file_path(SETTINGS_FILE))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Saves the settings (never from tests).
pub fn save_settings(settings: &AutoBackupSettings) -> std::io::Result<()> {
    if cfg!(test) {
        return Ok(());
    }
    let path = ui_state_file_path(SETTINGS_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    fs::write(path, payload)
}

/// Run history of the installed database (empty when none).
pub fn journal() -> BackupJournal {
    backup_dir()
        .map(|dir| BackupJournal::load(&dir))
        .unwrap_or_default()
}

/// A backup is running.
pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// Overall state shown on the Backups page and by the sidebar pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupStatus {
    UpToDate,
    /// Automatic backups are off and nothing failed.
    Disabled,
    /// No backup was ever written.
    Never,
    /// The last successful backup is older than [`OVERDUE_AFTER`].
    Overdue,
    /// The last run was written locally but missed a mirror.
    MirrorMissed,
    /// The last run wrote nothing.
    Failed,
}

impl BackupStatus {
    /// Worth a pill in the sidebar.
    pub fn needs_attention(self) -> bool {
        matches!(self, Self::Overdue | Self::MirrorMissed | Self::Failed)
    }
}

/// Age after which the last successful backup is flagged.
pub const OVERDUE_AFTER: time::Duration = time::Duration::days(7);

pub fn status(
    settings: &AutoBackupSettings,
    journal: &BackupJournal,
    now: OffsetDateTime,
) -> BackupStatus {
    match journal.last_run().map(|run| &run.outcome) {
        Some(RunOutcome::Success) | None => {}
        Some(RunOutcome::Partial) => return BackupStatus::MirrorMissed,
        Some(_) => return BackupStatus::Failed,
    }
    if settings.schedule == auto_backup::BackupSchedule::Off {
        return BackupStatus::Disabled;
    }
    match journal.last_success() {
        None => BackupStatus::Never,
        Some(run) if now - run.finished_at > OVERDUE_AFTER => BackupStatus::Overdue,
        Some(_) => BackupStatus::UpToDate,
    }
}

/// Checks mirror folders on a worker thread (an automounted share can stall) and reports
/// on the GTK main loop.
pub fn check_mirrors(
    mirrors: Vec<PathBuf>,
    on_done: impl FnOnce(Vec<(PathBuf, auto_backup::DestinationState)>) + 'static,
) {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let states = mirrors
            .into_iter()
            .map(|mirror| {
                let state = auto_backup::store::check_mirror(&mirror);
                (mirror, state)
            })
            .collect::<Vec<_>>();
        let _ = sender.send(states);
    });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(states) = receiver.await {
            on_done(states);
        }
    });
}

/// Restore test of the latest local backup with the session key, on a worker thread.
/// `on_done` gets the tested backup and its health, or `None` when there is nothing to test
/// or the test failed (logged).
pub fn test_latest_restore(
    session_key: &[u8],
    on_done: impl FnOnce(Result<(auto_backup::BackupFile, auto_backup::DatabaseHealth), ()>) + 'static,
) {
    let Some(context) = CONTEXT.get() else {
        on_done(Err(()));
        return;
    };
    if session_key.is_empty() {
        on_done(Err(()));
        return;
    }
    let key = Zeroizing::new(session_key.to_vec());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let secret = SecretBox::new(Box::new(key.to_vec()));
        drop(key);
        let backup_dir = backup_dir_for(&context.db_path);
        let result = match auto_backup::latest_local_backup(&backup_dir) {
            Ok(Some(file)) => context
                .runtime
                .block_on(auto_backup::health::test_restore(
                    &file.path,
                    &secret,
                    &backup_dir.join(".restore-test"),
                ))
                .map(|health| (file, health))
                .map_err(|error| warn!(%error, "restore test failed")),
            Ok(None) => Err(()),
            Err(error) => {
                warn!(%error, "backup folder unreadable");
                Err(())
            }
        };
        let _ = sender.send(result);
    });
    glib::MainContext::default().spawn_local(async move {
        on_done(receiver.await.unwrap_or(Err(())));
    });
}

/// Clears the running flag even if the worker panics.
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

async fn run(
    context: &Context,
    user_id: Uuid,
    key: SecretBox<Vec<u8>>,
    trigger: BackupTrigger,
) -> Option<BackupRunRecord> {
    let settings = load_settings();
    let backup_dir = backup_dir_for(&context.db_path);
    let journal = BackupJournal::load(&backup_dir);
    let fingerprint = if trigger == BackupTrigger::Close {
        match auto_backup::content_fingerprint(&context.pool).await {
            Ok(fingerprint) => Some(fingerprint),
            Err(error) => {
                warn!(%error, "backup fingerprint unavailable");
                return None;
            }
        }
    } else {
        None
    };
    if !auto_backup::is_due(
        &settings,
        &journal,
        trigger,
        OffsetDateTime::now_utc(),
        fingerprint.as_deref(),
    ) {
        return None;
    }
    let users = SqlxUserRepository::new(context.pool.clone());
    let actor = match users.get_by_id(user_id).await {
        Ok(Some(actor)) => actor,
        Ok(None) => return None,
        Err(error) => {
            warn!(%error, "backup actor lookup failed");
            return None;
        }
    };
    let envelopes = match auto_backup::load_envelopes(&users, user_id).await {
        Ok(envelopes) => envelopes,
        Err(error) => {
            warn!(%error, "backup envelopes unavailable");
            return None;
        }
    };
    // An account not yet migrated to an account key cannot be backed up automatically (its
    // envelope holds the key itself); skip quietly instead of reporting a daily failure.
    let migrated = heelonvault_core::services::auth_service::decode_password_envelope(
        &envelopes.password_envelope,
    )
    .is_ok_and(|envelope| envelope.is_account_key());
    if !migrated {
        info!("automatic backups need an account key; skipped for this account");
        return None;
    }
    let request = BackupRequest::new(&context.db_path, &backup_dir, &settings, trigger);
    match auto_backup::run_backup(&context.pool, &actor, &key, &envelopes, request).await {
        Ok(record) => Some(record),
        // Not an administrator: automatic backups do not apply to this account.
        Err(error) => {
            info!(%error, "automatic backup not available for this account");
            None
        }
    }
}

/// Starts a backup for `trigger` in the background when it is due. `on_done` runs on the
/// GTK main loop with the run record, or `None` when nothing ran (not due, already running,
/// not installed, not available for this account).
pub fn request(
    user_id: Uuid,
    session_key: &[u8],
    trigger: BackupTrigger,
    on_done: impl FnOnce(Option<BackupRunRecord>) + 'static,
) {
    let Some(context) = CONTEXT.get() else {
        on_done(None);
        return;
    };
    if session_key.is_empty() || RUNNING.swap(true, Ordering::SeqCst) {
        on_done(None);
        return;
    }
    let key = Zeroizing::new(session_key.to_vec());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let handle = std::thread::spawn(move || {
        let _running = RunningGuard;
        let secret = SecretBox::new(Box::new(key.to_vec()));
        drop(key);
        let record = context
            .runtime
            .block_on(run(context, user_id, secret, trigger));
        let _ = sender.send(record);
    });
    if let Ok(mut pending) = PENDING.lock() {
        *pending = Some(handle);
    }
    glib::MainContext::default().spawn_local(async move {
        on_done(receiver.await.ok().flatten());
    });
}

/// Waits, at most `timeout`, for a running backup to finish (called when quitting).
pub fn wait_for_pending(timeout: Duration) {
    let handle = PENDING.lock().ok().and_then(|mut pending| pending.take());
    let Some(handle) = handle else {
        return;
    };
    let deadline = Instant::now() + timeout;
    while !handle.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    if handle.is_finished() {
        let _ = handle.join();
    } else {
        warn!("quitting while an automatic backup is still running");
    }
}

/// Toast for a finished run.
pub fn toast_message(record: &BackupRunRecord) -> String {
    match record.outcome {
        RunOutcome::Success => heelonvault_core::tr!("auto-backup-toast-success"),
        RunOutcome::Partial => heelonvault_core::tr!("auto-backup-toast-partial"),
        _ => heelonvault_core::tr!("auto-backup-toast-failed"),
    }
}

/// Toast shown at unlock when no backup was due but the latest run did not fully succeed
/// (e.g. the one started when the previous session closed).
pub fn unlock_reminder(journal: &BackupJournal) -> Option<String> {
    let last = journal.last_run()?;
    match last.outcome {
        RunOutcome::Success => None,
        RunOutcome::Partial => Some(heelonvault_core::tr!("auto-backup-toast-last-partial")),
        _ => Some(heelonvault_core::tr!("auto-backup-toast-last-failed")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_live_next_to_the_database() {
        assert_eq!(
            backup_dir_for(Path::new(
                "/home/u/.local/share/heelonvault/heelonvault-rust.db"
            )),
            PathBuf::from("/home/u/.local/share/heelonvault/backups")
        );
        assert_eq!(
            backup_dir_for(Path::new("vault.db")),
            PathBuf::from("backups")
        );
    }

    /// Synthetic account in a migrated temporary database (fast KDF, no real data).
    fn seed(
        runtime: &tokio::runtime::Runtime,
        db_path: &Path,
    ) -> Result<(SqlitePool, Uuid, SecretBox<Vec<u8>>), Box<dyn std::error::Error>> {
        use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
        use heelonvault_core::services::auth_service::AuthServiceImpl;
        use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
        use heelonvault_core::services::crypto_service::{CryptoServiceImpl, KdfConfig};
        let crypto = || {
            CryptoServiceImpl::new(KdfConfig {
                memory_cost_kib: 1024,
                time_cost: 1,
                parallelism: 1,
                output_len: 32,
            })
        };
        runtime.block_on(async {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(db_path)
                        .create_if_missing(true),
                )
                .await?;
            let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
            sqlx::migrate::Migrator::new(migrations.as_path())
                .await?
                .run(&pool)
                .await?;
            let backup = BackupServiceImpl::new();
            let phrase = backup.generate_recovery_key()?.recovery_phrase;
            let boot = bootstrap_first_admin_with_recovery(
                &SqlxUserRepository::new(pool.clone()),
                &AuthServiceImpl::new(crypto()),
                &backup,
                &crypto(),
                "synthetic",
                SecretBox::new(Box::new(b"synthetic-master-password-2026".to_vec())),
                phrase,
            )
            .await?;
            Ok((pool, boot.user_id, boot.master_key))
        })
    }

    #[test]
    fn unlock_and_close_triggers_back_up_only_when_due() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("heelonvault-auto-backup-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?,
        );
        let db_path = dir.join("vault.db");
        let (pool, user_id, key) = seed(&runtime, &db_path)?;
        let context = Context {
            runtime: Arc::clone(&runtime),
            pool,
            db_path,
        };
        let copy = |key: &SecretBox<Vec<u8>>| {
            SecretBox::new(Box::new(secrecy::ExposeSecret::expose_secret(key).clone()))
        };

        let first = runtime.block_on(run(&context, user_id, copy(&key), BackupTrigger::Unlock));
        let first = first.ok_or("first unlock backs up")?;
        assert!(first.succeeded());
        let name = first.file_name.clone().ok_or("file name")?;
        assert!(dir.join("backups").join(name).is_file());
        assert!(
            runtime
                .block_on(run(&context, user_id, copy(&key), BackupTrigger::Unlock))
                .is_none(),
            "not due again within the day"
        );
        assert!(
            runtime
                .block_on(run(&context, user_id, copy(&key), BackupTrigger::Close))
                .is_none(),
            "nothing changed since"
        );
        assert_eq!(journal_at(&dir).runs.len(), 1);
        assert!(
            runtime
                .block_on(run(
                    &context,
                    Uuid::new_v4(),
                    copy(&key),
                    BackupTrigger::Manual
                ))
                .is_none(),
            "unknown account"
        );
        runtime.block_on(context.pool.close());
        fs::remove_dir_all(&dir)?;
        Ok(())
    }

    fn journal_at(dir: &Path) -> BackupJournal {
        BackupJournal::load(&dir.join("backups"))
    }

    /// A journal record; `BackupRunRecord` is `#[non_exhaustive]`, so it is read from JSON.
    fn record(
        outcome: &str,
        finished_at: OffsetDateTime,
    ) -> Result<BackupRunRecord, Box<dyn std::error::Error>> {
        let at = finished_at.format(&time::format_description::well_known::Rfc3339)?;
        Ok(serde_json::from_str(&format!(
            r#"{{"started_at":"{at}","finished_at":"{at}","trigger":"manual","outcome":{outcome},
            "file_name":null,"size":0,"sha256":null,"fingerprint":null,"health":null,
            "mirrors":[],"pruned":0}}"#
        ))?)
    }

    #[test]
    fn the_status_flags_failures_mirrors_and_age() -> Result<(), Box<dyn std::error::Error>> {
        let now = OffsetDateTime::now_utc();
        let settings = AutoBackupSettings::default();
        let mut journal = BackupJournal::default();
        assert_eq!(status(&settings, &journal, now), BackupStatus::Never);
        let success = r#"{"result":"success"}"#;
        journal
            .runs
            .push(record(success, now - time::Duration::days(1))?);
        assert_eq!(status(&settings, &journal, now), BackupStatus::UpToDate);
        assert_eq!(
            status(&settings, &journal, now + time::Duration::days(7)),
            BackupStatus::Overdue
        );
        journal.runs.push(record(r#"{"result":"partial"}"#, now)?);
        assert_eq!(status(&settings, &journal, now), BackupStatus::MirrorMissed);
        journal
            .runs
            .push(record(r#"{"result":"failed","detail":"x"}"#, now)?);
        assert_eq!(status(&settings, &journal, now), BackupStatus::Failed);
        assert!(BackupStatus::Failed.needs_attention());
        let mut off = AutoBackupSettings::default();
        off.schedule = auto_backup::BackupSchedule::Off;
        assert_eq!(
            status(&off, &BackupJournal::default(), now),
            BackupStatus::Disabled
        );
        assert!(!BackupStatus::Disabled.needs_attention());
        Ok(())
    }

    #[test]
    fn requests_are_ignored_until_installed() {
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = std::rc::Rc::clone(&called);
        if CONTEXT.get().is_none() {
            request(
                Uuid::new_v4(),
                &[1; 32],
                BackupTrigger::Manual,
                move |record| {
                    assert!(record.is_none());
                    flag.set(true);
                },
            );
            assert!(called.get(), "answered synchronously");
            assert!(!RUNNING.load(Ordering::SeqCst));
        }
    }
}

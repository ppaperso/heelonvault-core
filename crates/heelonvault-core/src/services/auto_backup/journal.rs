//! Run history (`backup-journal.json` in the local backup folder). It holds no secret: dates,
//! file names, digests, sizes, destination paths and health counters.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::errors::AppError;
use crate::utils::private_fs;

use super::health::DatabaseHealth;

const JOURNAL_FILE: &str = "backup-journal.json";
const JOURNAL_MAX_RUNS: usize = 200;

/// What started a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BackupTrigger {
    /// First unlock after the daily interval elapsed.
    Unlock,
    /// Closing or locking the session after changes.
    Close,
    /// "Back up now".
    Manual,
    /// Right after a master-password change: older backups open with the old password.
    PasswordChange,
}

/// Result for one mirror folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail", rename_all = "snake_case")]
#[non_exhaustive]
pub enum MirrorStatus {
    /// Copied and read back identically.
    Copied,
    /// Folder absent (share not mounted); nothing written.
    Missing,
    NotADirectory,
    /// Folder unreadable, or the copy failed; the OS message.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MirrorOutcome {
    pub path: PathBuf,
    pub status: MirrorStatus,
    /// Backups deleted by the retention policy in this mirror.
    #[serde(default)]
    pub pruned: u32,
}

/// Overall result of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", content = "detail", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RunOutcome {
    /// Local backup written and verified, every mirror copied.
    Success,
    /// Local backup written and verified, at least one mirror missed.
    Partial,
    /// No backup was written; the reason (no secret in it).
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BackupRunRecord {
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub finished_at: OffsetDateTime,
    pub trigger: BackupTrigger,
    pub outcome: RunOutcome,
    /// Backup file name, identical in every destination.
    pub file_name: Option<String>,
    /// Size of the encrypted file.
    pub size: u64,
    /// SHA-256 of the encrypted file (never of the plaintext).
    pub sha256: Option<String>,
    /// Content fingerprint at backup time, to skip a close-time backup when nothing changed.
    pub fingerprint: Option<String>,
    pub health: Option<DatabaseHealth>,
    pub mirrors: Vec<MirrorOutcome>,
    /// Local backups deleted by the retention policy.
    pub pruned: u32,
}

impl BackupRunRecord {
    /// A backup was written and verified locally.
    pub fn succeeded(&self) -> bool {
        matches!(self.outcome, RunOutcome::Success | RunOutcome::Partial)
    }
}

/// Runs, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BackupJournal {
    pub runs: Vec<BackupRunRecord>,
}

impl BackupJournal {
    pub fn path(backup_dir: &Path) -> PathBuf {
        backup_dir.join(JOURNAL_FILE)
    }

    /// Reads the journal; an absent or unreadable one is an empty history (the backups
    /// themselves are still listed from the folders).
    pub fn load(backup_dir: &Path) -> Self {
        fs::read(Self::path(backup_dir))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn last_success(&self) -> Option<&BackupRunRecord> {
        self.runs.iter().rev().find(|run| run.succeeded())
    }

    pub fn last_run(&self) -> Option<&BackupRunRecord> {
        self.runs.last()
    }

    /// Appends a run and rewrites the file atomically, keeping the latest runs only.
    pub fn append(backup_dir: &Path, record: BackupRunRecord) -> Result<Self, AppError> {
        let mut journal = Self::load(backup_dir);
        journal.runs.push(record);
        let excess = journal.runs.len().saturating_sub(JOURNAL_MAX_RUNS);
        journal.runs.drain(..excess);
        let bytes = serde_json::to_vec_pretty(&journal)
            .map_err(|err| AppError::Storage(format!("failed to serialize journal: {err}")))?;
        private_fs::create_private_dir_all(backup_dir).map_err(AppError::Io)?;
        let temp = backup_dir.join(format!(".{JOURNAL_FILE}.partial"));
        private_fs::write_private(&temp, &bytes).map_err(AppError::Io)?;
        let target = Self::path(backup_dir);
        // Windows refuses to rename onto an existing file.
        #[cfg(windows)]
        if target.exists() {
            fs::remove_file(&target).map_err(AppError::Io)?;
        }
        fs::rename(&temp, &target).map_err(|err| {
            let _ = fs::remove_file(&temp);
            AppError::Io(err)
        })?;
        Ok(journal)
    }
}

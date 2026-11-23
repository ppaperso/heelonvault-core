//! Backup files on disk: naming, verified writes, destination checks, listing and pruning.
//!
//! Everything here is blocking file I/O. A mirror can be an automounted network share that
//! stalls while it mounts: callers run these functions off the UI thread and stop waiting
//! after a timeout.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use time::{Date, Month, OffsetDateTime, Time};

use crate::errors::AppError;
use crate::utils::private_fs;

use super::retention::RetentionPolicy;

/// Extension of automatic backups. Only files named `heelonvault-<stamp>.hvba` are ever
/// listed or pruned, so a destination folder can hold other files safely.
pub const BACKUP_EXTENSION: &str = "hvba";
const FILE_PREFIX: &str = "heelonvault-";

/// One automatic backup found in a destination.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackupFile {
    pub path: PathBuf,
    pub created_at: OffsetDateTime,
    pub size: u64,
}

/// Whether a mirror folder can receive backups.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DestinationState {
    Ready,
    /// The folder does not exist, e.g. a network share that is not mounted. The folder is
    /// never created: on an unmounted mount point that would silently fill the local disk.
    Missing,
    NotADirectory,
    /// The folder exists but cannot be read (permissions, I/O error); the OS message.
    Unreachable(String),
}

/// `heelonvault-20261123T081530Z.hvba`.
pub fn file_name_for(created_at: OffsetDateTime) -> String {
    let at = created_at.to_offset(time::UtcOffset::UTC);
    format!(
        "{FILE_PREFIX}{:04}{:02}{:02}T{:02}{:02}{:02}Z.{BACKUP_EXTENSION}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// Inverse of [`file_name_for`]; `None` for any other file name.
pub fn parse_file_name(name: &str) -> Option<OffsetDateTime> {
    let stamp = name
        .strip_prefix(FILE_PREFIX)?
        .strip_suffix(&format!(".{BACKUP_EXTENSION}"))?;
    let bytes = stamp.as_bytes();
    if bytes.len() != 16
        || bytes[8] != b'T'
        || bytes[15] != b'Z'
        || !stamp[..8].bytes().all(|b| b.is_ascii_digit())
        || !stamp[9..15].bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| stamp[range].parse::<u32>().ok();
    let year = i32::try_from(number(0..4)?).ok()?;
    let month = Month::try_from(u8::try_from(number(4..6)?).ok()?).ok()?;
    let day = u8::try_from(number(6..8)?).ok()?;
    let hour = u8::try_from(number(9..11)?).ok()?;
    let minute = u8::try_from(number(11..13)?).ok()?;
    let second = u8::try_from(number(13..15)?).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let time = Time::from_hms(hour, minute, second).ok()?;
    Some(date.with_time(time).assume_utc())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Checks a mirror folder without creating it.
pub fn check_mirror(path: &Path) -> DestinationState {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => match fs::read_dir(path) {
            Ok(_) => DestinationState::Ready,
            Err(err) => DestinationState::Unreachable(err.to_string()),
        },
        Ok(_) => DestinationState::NotADirectory,
        Err(err) if err.kind() == io::ErrorKind::NotFound => DestinationState::Missing,
        Err(err) => DestinationState::Unreachable(err.to_string()),
    }
}

/// Creates the local backup folder (owner-only) when missing.
pub fn ensure_local_dir(path: &Path) -> Result<(), AppError> {
    private_fs::create_private_dir_all(path).map_err(AppError::Io)
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Requested on creation only: network shares may force their own mode, and the
        // content is encrypted anyway.
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Writes `bytes` as `dir/name` through a temporary file, then reads the file back and
/// compares its SHA-256. Returns the digest. An existing backup is never overwritten.
pub fn write_verified(dir: &Path, name: &str, bytes: &[u8]) -> Result<String, AppError> {
    let target = dir.join(name);
    if target.exists() {
        return Err(AppError::Conflict(format!(
            "backup {name} already exists in this destination"
        )));
    }
    let temp = dir.join(format!(".{name}.partial"));
    if temp.exists() {
        fs::remove_file(&temp).map_err(AppError::Io)?;
    }
    let expected = sha256_hex(bytes);
    let result = write_new(&temp, bytes)
        .and_then(|()| fs::rename(&temp, &target))
        .map_err(AppError::Io);
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    let written = fs::read(&target).map_err(AppError::Io)?;
    if sha256_hex(&written) != expected {
        let _ = fs::remove_file(&target);
        return Err(AppError::Storage(format!(
            "backup {name} does not read back identically"
        )));
    }
    Ok(expected)
}

/// Automatic backups of a destination, newest first.
pub fn list_backups(dir: &Path) -> Result<Vec<BackupFile>, AppError> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(AppError::Io)? {
        let entry = entry.map_err(AppError::Io)?;
        let Some(created_at) = entry.file_name().to_str().and_then(parse_file_name) else {
            continue;
        };
        let metadata = entry.metadata().map_err(AppError::Io)?;
        if metadata.is_file() {
            files.push(BackupFile {
                path: entry.path(),
                created_at,
                size: metadata.len(),
            });
        }
    }
    files.sort_by_key(|file| std::cmp::Reverse(file.created_at));
    Ok(files)
}

/// Deletes the backups the policy does not keep. Returns the deleted paths.
pub fn prune(dir: &Path, policy: &RetentionPolicy) -> Result<Vec<PathBuf>, AppError> {
    let files = list_backups(dir)?;
    let created: Vec<OffsetDateTime> = files.iter().map(|file| file.created_at).collect();
    let keep = policy.keep(&created);
    let mut removed = Vec::new();
    for (index, file) in files.into_iter().enumerate() {
        if !keep.contains(&index) {
            fs::remove_file(&file.path).map_err(AppError::Io)?;
            removed.push(file.path);
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use time::Duration;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::days(20_000) + Duration::seconds(seconds)
    }

    #[test]
    fn file_names_round_trip_and_ignore_foreign_files() {
        let created = at(3_723);
        let name = file_name_for(created);
        assert_eq!(name, "heelonvault-20241004T010203Z.hvba");
        assert_eq!(parse_file_name(&name), Some(created));
        for foreign in [
            "heelonvault-20240501T010203Z.hvb",
            "heelonvault-2024-05-01.hvba",
            "notes.hvba",
            ".heelonvault-20240501T010203Z.hvba.partial",
            "heelonvault-20241301T010203Z.hvba",
        ] {
            assert_eq!(parse_file_name(foreign), None, "{foreign}");
        }
    }

    #[test]
    fn a_missing_mirror_is_reported_and_never_created() -> Result<(), AppError> {
        let dir = TempDir::new().map_err(AppError::Io)?;
        let mount = dir.path().join("beestation").join("HeelonVault");
        assert_eq!(check_mirror(&mount), DestinationState::Missing);
        assert!(!mount.exists());
        let file = dir.path().join("file");
        fs::write(&file, b"x").map_err(AppError::Io)?;
        assert_eq!(check_mirror(&file), DestinationState::NotADirectory);
        assert_eq!(check_mirror(dir.path()), DestinationState::Ready);
        Ok(())
    }

    #[test]
    fn verified_writes_never_overwrite_and_leave_no_partial_file() -> Result<(), AppError> {
        let dir = TempDir::new().map_err(AppError::Io)?;
        let name = file_name_for(at(0));
        let digest = write_verified(dir.path(), &name, b"payload")?;
        assert_eq!(digest, sha256_hex(b"payload"));
        assert!(matches!(
            write_verified(dir.path(), &name, b"other"),
            Err(AppError::Conflict(_))
        ));
        assert_eq!(
            fs::read(dir.path().join(&name)).map_err(AppError::Io)?,
            b"payload"
        );
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .map_err(AppError::Io)?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".partial"))
            .collect();
        assert!(leftovers.is_empty());
        Ok(())
    }

    #[test]
    fn pruning_only_touches_automatic_backups() -> Result<(), AppError> {
        let dir = TempDir::new().map_err(AppError::Io)?;
        for day in 0..10 {
            write_verified(dir.path(), &file_name_for(at(day * 86_400)), b"backup")?;
        }
        fs::write(dir.path().join("manual-export.hvb"), b"keep").map_err(AppError::Io)?;
        let removed = prune(dir.path(), &RetentionPolicy::new(3, 0, 0))?;
        assert_eq!(removed.len(), 7);
        let left = list_backups(dir.path())?;
        assert_eq!(left.len(), 3);
        assert_eq!(left[0].created_at, at(9 * 86_400), "newest first");
        assert!(dir.path().join("manual-export.hvb").exists());
        Ok(())
    }
}

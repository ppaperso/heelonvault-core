//! Files and directories reachable only by their owner.
//!
//! Unix sets explicit modes, independent of the process umask. Windows relies on the ACL the
//! user profile already grants (owner, SYSTEM, Administrators), so no mode is applied there.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

#[cfg(unix)]
const OWNER_ONLY_DIR: u32 = 0o700;
#[cfg(unix)]
const OWNER_ONLY_FILE: u32 = 0o600;
#[cfg(unix)]
const GROUP_SHARED_FILE: u32 = 0o660;

/// Creates missing directories as owner-only. Existing directories are never modified: they
/// may be the user's home or a directory shared on purpose.
pub fn create_private_dir_all(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(OWNER_ONLY_DIR);
    builder.create(path)
}

/// Writes `contents` to a file readable by its owner only, also when the file already existed.
pub fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(OWNER_ONLY_FILE);
    let mut file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(OWNER_ONLY_FILE))?;
    file.write_all(contents)?;
    file.sync_all()
}

/// Options for a new file the application owns: owner-only, or owner and group when the parent
/// directory is group-writable (a data directory shared on purpose, by group or ACL).
#[cfg(unix)]
pub fn app_open_options(path: &Path) -> io::Result<OpenOptions> {
    let mut options = OpenOptions::new();
    options.mode(if parent_is_group_shared(path)? {
        GROUP_SHARED_FILE
    } else {
        OWNER_ONLY_FILE
    });
    Ok(options)
}

#[cfg(not(unix))]
pub fn app_open_options(_path: &Path) -> io::Result<OpenOptions> {
    Ok(OpenOptions::new())
}

/// Removes access an application file never needs: always "others", and "group" unless the
/// parent directory is group-writable. Never adds a permission. Returns whether it changed.
#[cfg(unix)]
pub fn tighten_app_file(path: &Path) -> io::Result<bool> {
    let mode = fs::metadata(path)?.permissions().mode() & 0o7777;
    let allowed = if parent_is_group_shared(path)? {
        GROUP_SHARED_FILE
    } else {
        OWNER_ONLY_FILE
    };
    let tightened = mode & allowed;
    if tightened == mode {
        return Ok(false);
    }
    fs::set_permissions(path, fs::Permissions::from_mode(tightened))?;
    Ok(true)
}

#[cfg(not(unix))]
pub fn tighten_app_file(_path: &Path) -> io::Result<bool> {
    Ok(false)
}

/// Whether users other than the owner and its group can enter or read `path`.
#[cfg(unix)]
pub fn is_world_accessible(path: &Path) -> io::Result<bool> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o007 != 0)
}

#[cfg(not(unix))]
pub fn is_world_accessible(_path: &Path) -> io::Result<bool> {
    Ok(false)
}

#[cfg(unix)]
fn parent_is_group_shared(path: &Path) -> io::Result<bool> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    Ok(fs::metadata(parent)?.permissions().mode() & 0o020 != 0)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;

    use super::write_private;

    #[test]
    fn write_private_replaces_previous_contents() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("report.txt");
        fs::write(&path, b"previous and longer contents")?;

        write_private(&path, b"new")?;

        assert_eq!(fs::read(&path)?, b"new");
        Ok(())
    }

    #[cfg(unix)]
    mod unix {
        use std::fs;
        use std::io;
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;

        use super::super::{
            app_open_options, create_private_dir_all, is_world_accessible, tighten_app_file,
            write_private,
        };

        fn mode(path: &Path) -> io::Result<u32> {
            Ok(fs::metadata(path)?.permissions().mode() & 0o777)
        }

        fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))
        }

        #[test]
        fn created_directories_are_owner_only() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            let nested = root.path().join("data").join("logs");

            create_private_dir_all(&nested)?;

            assert_eq!(mode(&root.path().join("data"))?, 0o700);
            assert_eq!(mode(&nested)?, 0o700);
            Ok(())
        }

        #[test]
        fn existing_directories_are_left_untouched() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            let existing = root.path().join("shared");
            fs::create_dir(&existing)?;
            set_mode(&existing, 0o755)?;

            create_private_dir_all(&existing)?;

            assert_eq!(mode(&existing)?, 0o755);
            Ok(())
        }

        #[test]
        fn write_private_restricts_an_existing_readable_file() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            let path = root.path().join("export.hvb");
            fs::write(&path, b"old")?;
            set_mode(&path, 0o644)?;

            write_private(&path, b"new")?;

            assert_eq!(mode(&path)?, 0o600);
            Ok(())
        }

        #[test]
        fn new_app_file_is_owner_only_in_a_private_directory() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            set_mode(root.path(), 0o700)?;
            let path = root.path().join("vault.db");

            app_open_options(&path)?
                .write(true)
                .create_new(true)
                .open(&path)?;

            assert_eq!(mode(&path)?, 0o600);
            Ok(())
        }

        #[test]
        fn tightening_removes_group_and_others_outside_shared_directories() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            let dir = root.path().join("data");
            fs::create_dir(&dir)?;
            set_mode(&dir, 0o755)?;
            let path = dir.join("vault.db");
            fs::write(&path, b"")?;
            set_mode(&path, 0o644)?;

            assert!(tighten_app_file(&path)?);
            assert_eq!(mode(&path)?, 0o600);
            Ok(())
        }

        #[test]
        fn tightening_keeps_group_access_in_a_shared_directory() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            let dir = root.path().join("shared");
            fs::create_dir(&dir)?;
            set_mode(&dir, 0o770)?;
            let path = dir.join("vault.db");
            fs::write(&path, b"")?;
            set_mode(&path, 0o664)?;

            assert!(tighten_app_file(&path)?);
            assert_eq!(mode(&path)?, 0o660);
            Ok(())
        }

        #[test]
        fn tightening_never_adds_permissions() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            set_mode(root.path(), 0o700)?;
            let path = root.path().join("vault.db");
            fs::write(&path, b"")?;
            set_mode(&path, 0o400)?;

            assert!(!tighten_app_file(&path)?);
            assert_eq!(mode(&path)?, 0o400);
            Ok(())
        }

        #[test]
        fn world_access_is_detected() -> io::Result<()> {
            let root = tempfile::tempdir()?;
            let dir = root.path().join("data");
            fs::create_dir(&dir)?;

            set_mode(&dir, 0o755)?;
            assert!(is_world_accessible(&dir)?);
            set_mode(&dir, 0o750)?;
            assert!(!is_world_accessible(&dir)?);
            Ok(())
        }
    }
}

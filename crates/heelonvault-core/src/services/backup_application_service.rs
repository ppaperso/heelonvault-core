use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use secrecy::SecretString;
use sqlx::SqlitePool;
use tracing::warn;
use uuid::Uuid;

use crate::errors::AppError;
use crate::repositories::user_repository::UserRepository;
use crate::services::access_control::{Action, Resource, check_permission};
use crate::services::backup_service::{BackupMetadata, BackupService};

/// Deletes the temporary snapshot even if the export fails or panics.
struct SnapshotGuard {
    path: PathBuf,
}

impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug, Clone)]
pub struct RotationBackupTicket {
    pub backup_file_path: String,
    pub recovery_phrase: SecretString,
    pub metadata_sha256_hex: String,
    pub created_at: String,
}

/// Application-level authorization wrapper for backup operations.
/// Enforces access control before delegating to the underlying backup service.
#[trait_variant::make(BackupApplicationService: Send)]
pub trait LocalBackupApplicationService {
    /// Export backup with authorization check (admin-only).
    async fn export_backup_secured(
        &self,
        actor_id: Uuid,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
    ) -> Result<BackupMetadata, AppError>;

    /// Restore backup with authorization check (admin-only).
    async fn restore_backup_secured(
        &self,
        actor_id: Uuid,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
        new_sqlite_db_path: &Path,
    ) -> Result<BackupMetadata, AppError>;

    /// Export backup for master-key rotation and keep recovery material in-memory.
    async fn export_rotation_backup_secured(
        &self,
        actor_id: Uuid,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
    ) -> Result<RotationBackupTicket, AppError>;

    /// Restore backup from a previously created rotation backup ticket.
    async fn restore_rotation_backup_secured(
        &self,
        actor_id: Uuid,
        ticket: &RotationBackupTicket,
        new_sqlite_db_path: &Path,
    ) -> Result<BackupMetadata, AppError>;
}

pub struct BackupApplicationServiceImpl<TUserRepo, TBackupSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TBackupSvc: BackupService + Send + Sync,
{
    user_repo: TUserRepo,
    backup_service: TBackupSvc,
    pool: SqlitePool,
}

impl<TUserRepo, TBackupSvc> BackupApplicationServiceImpl<TUserRepo, TBackupSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TBackupSvc: BackupService + Send + Sync,
{
    pub fn new(user_repo: TUserRepo, backup_service: TBackupSvc, pool: SqlitePool) -> Self {
        Self {
            user_repo,
            backup_service,
            pool,
        }
    }

    /// Produces a consistent copy of the live database through `VACUUM INTO`.
    ///
    /// Reading the `.db` file directly would miss everything still sitting in the `-wal`
    /// file, yielding a backup that decrypts cleanly but is missing recent writes.
    async fn snapshot_database(&self, sqlite_db_path: &Path) -> Result<SnapshotGuard, AppError> {
        let mut snapshot_path = sqlite_db_path.to_path_buf();
        let snapshot_name = match sqlite_db_path.file_name().and_then(|name| name.to_str()) {
            Some(name) => format!(".{name}.snapshot"),
            None => ".heelonvault.snapshot".to_string(),
        };
        snapshot_path.set_file_name(snapshot_name);

        // VACUUM INTO refuses to write onto an existing file.
        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path).map_err(AppError::Io)?;
        }

        let guard = SnapshotGuard {
            path: snapshot_path.clone(),
        };

        let snapshot_arg = snapshot_path
            .to_str()
            .ok_or_else(|| AppError::Validation("database path is not valid UTF-8".to_string()))?;
        // SQLite rejects bound parameters in `VACUUM INTO`, so the path must be inlined.
        // Rather than escaping, refuse any path that could break out of the literal.
        if snapshot_arg.contains('\'') || snapshot_arg.contains('\0') {
            return Err(AppError::Validation(
                "database path contains characters that cannot be used for a snapshot".to_string(),
            ));
        }
        // `raw_sql` runs the statement unprepared, which VACUUM requires.
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("VACUUM INTO '{snapshot_arg}'")))
            .execute(&self.pool)
            .await?;

        if !snapshot_path.exists() {
            return Err(AppError::Storage(
                "database snapshot was not produced".to_string(),
            ));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&snapshot_path, fs::Permissions::from_mode(0o600))
                .map_err(AppError::Io)?;
        }

        Ok(guard)
    }
}

impl<TUserRepo, TBackupSvc> BackupApplicationService
    for BackupApplicationServiceImpl<TUserRepo, TBackupSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TBackupSvc: BackupService + Send + Sync,
{
    async fn export_backup_secured(
        &self,
        actor_id: Uuid,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
    ) -> Result<BackupMetadata, AppError> {
        let actor = self
            .user_repo
            .get_by_id(actor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("actor user not found".to_string()))?;

        check_permission(&actor, Action::BackupExport, &Resource::Global).inspect_err(|_err| {
            warn!(actor_id = %actor_id, "backup export permission denied");
        })?;

        let snapshot = self.snapshot_database(sqlite_db_path).await?;

        self.backup_service.export_hvb_with_recovery_key(
            snapshot.path.as_path(),
            backup_file_path,
            recovery_phrase,
        )
    }

    async fn restore_backup_secured(
        &self,
        actor_id: Uuid,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
        new_sqlite_db_path: &Path,
    ) -> Result<BackupMetadata, AppError> {
        let actor = self
            .user_repo
            .get_by_id(actor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("actor user not found".to_string()))?;

        check_permission(&actor, Action::BackupRestore, &Resource::Global).inspect_err(|_err| {
            warn!(actor_id = %actor_id, "backup restore permission denied");
        })?;

        self.backup_service.import_hvb_with_recovery_key(
            backup_file_path,
            recovery_phrase,
            new_sqlite_db_path,
        )
    }

    async fn export_rotation_backup_secured(
        &self,
        actor_id: Uuid,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
    ) -> Result<RotationBackupTicket, AppError> {
        let recovery = self.backup_service.generate_recovery_key()?;
        let metadata = BackupApplicationService::export_backup_secured(
            self,
            actor_id,
            sqlite_db_path,
            backup_file_path,
            &recovery.recovery_phrase,
        )
        .await?;

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_err| AppError::Internal)?
            .as_secs()
            .to_string();

        Ok(RotationBackupTicket {
            backup_file_path: backup_file_path.to_string_lossy().to_string(),
            recovery_phrase: recovery.recovery_phrase,
            metadata_sha256_hex: metadata.sha256_hex,
            created_at,
        })
    }

    async fn restore_rotation_backup_secured(
        &self,
        actor_id: Uuid,
        ticket: &RotationBackupTicket,
        new_sqlite_db_path: &Path,
    ) -> Result<BackupMetadata, AppError> {
        BackupApplicationService::restore_backup_secured(
            self,
            actor_id,
            Path::new(ticket.backup_file_path.as_str()),
            &ticket.recovery_phrase,
            new_sqlite_db_path,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use std::collections::HashMap;
    use std::sync::MutexGuard;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    use crate::errors::AppError;
    use crate::models::{User, UserRole};
    use crate::repositories::user_repository::UserRepository;
    use crate::services::backup_service::{BackupMetadata, BackupService};

    use super::{BackupApplicationService, BackupApplicationServiceImpl};

    /// These tests cover authorization only. A file-backed pool is required because
    /// `VACUUM INTO` produces nothing from an in-memory database.
    fn unique_db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("heelonvault-src-{}.db", Uuid::new_v4()))
    }

    async fn test_pool() -> sqlx::SqlitePool {
        let path = std::env::temp_dir().join(format!("heelonvault-rbac-{}.db", Uuid::new_v4()));
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite://{}?mode=rwc", path.display()))
            .await
            .expect("file-backed sqlite pool")
    }

    #[derive(Default, Clone)]
    struct StubUserRepo {
        users: Arc<Mutex<HashMap<Uuid, User>>>,
    }

    impl StubUserRepo {
        fn lock_users(&self) -> Result<MutexGuard<'_, HashMap<Uuid, User>>, AppError> {
            self.users.lock().map_err(|_| AppError::Internal)
        }

        fn insert_user(&self, id: Uuid, role: UserRole) {
            if let Ok(mut users) = self.users.lock() {
                users.insert(
                    id,
                    User {
                        id,
                        username: format!("user_{}", id),
                        role,
                        email: None,
                        display_name: None,
                        preferred_language: "fr".to_string(),
                        show_passwords_in_edit: false,
                        updated_at: None,
                    },
                );
            }
        }
    }

    impl UserRepository for StubUserRepo {
        async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
            Ok(self.lock_users()?.get(&id).cloned())
        }
        async fn get_by_username(&self, _: &str) -> Result<Option<User>, AppError> {
            Ok(None)
        }
        async fn resolve_username_for_login_identifier(
            &self,
            _: &str,
        ) -> Result<Option<String>, AppError> {
            Ok(None)
        }
        async fn list_all(&self) -> Result<Vec<User>, AppError> {
            Ok(vec![])
        }
        async fn create_user_db(&self, _: Uuid, _: &str, _: &UserRole) -> Result<(), AppError> {
            Ok(())
        }
        async fn delete_user(&self, _: Uuid) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_user_role(&self, _: Uuid, _: &UserRole) -> Result<(), AppError> {
            Ok(())
        }
        async fn list_all_password_envelopes(&self) -> Result<Vec<(String, Vec<u8>)>, AppError> {
            Ok(vec![])
        }
        async fn get_password_envelope_by_user_id(
            &self,
            _: Uuid,
        ) -> Result<Option<secrecy::SecretBox<Vec<u8>>>, AppError> {
            Ok(None)
        }
        async fn update_user_profile(
            &self,
            _: Uuid,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<bool>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_password_envelope(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_totp_secret_envelope(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_show_passwords_in_edit(&self, _: Uuid, _: bool) -> Result<(), AppError> {
            Ok(())
        }
        async fn get_recovery_phrase_envelope(
            &self,
            _: Uuid,
        ) -> Result<Option<secrecy::SecretBox<Vec<u8>>>, AppError> {
            Ok(None)
        }
        async fn set_recovery_phrase_envelope(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn get_recovery_verifier(&self, _: Uuid) -> Result<Option<Vec<u8>>, AppError> {
            Ok(None)
        }
        async fn set_recovery_verifier(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[derive(Default, Clone)]
    struct StubBackupService;

    impl BackupService for StubBackupService {
        fn generate_recovery_key(
            &self,
        ) -> Result<crate::services::backup_service::RecoveryKeyBundle, AppError> {
            Ok(crate::services::backup_service::RecoveryKeyBundle {
                recovery_phrase: secrecy::SecretString::new(
                    "test recovery phrase".to_string().into(),
                ),
            })
        }
        fn build_recovery_verifier(
            &self,
            _: &secrecy::SecretString,
        ) -> Result<secrecy::SecretBox<Vec<u8>>, AppError> {
            Ok(secrecy::SecretBox::new(Box::new(vec![0_u8; 65])))
        }
        fn verify_recovery_phrase(
            &self,
            _: &secrecy::SecretString,
            _: &[u8],
        ) -> Result<bool, AppError> {
            Ok(true)
        }
        fn export_hvb_with_recovery_key(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: &secrecy::SecretString,
        ) -> Result<BackupMetadata, AppError> {
            Ok(BackupMetadata {
                sha256_hex: "abc123".to_string(),
                plaintext_size: 1024,
            })
        }
        fn import_hvb_with_recovery_key(
            &self,
            _: &std::path::Path,
            _: &secrecy::SecretString,
            _: &std::path::Path,
        ) -> Result<BackupMetadata, AppError> {
            Ok(BackupMetadata {
                sha256_hex: "def456".to_string(),
                plaintext_size: 2048,
            })
        }
    }

    #[tokio::test]
    async fn admin_can_export_backup() {
        let user_repo = StubUserRepo::default();
        let admin_id = Uuid::new_v4();
        user_repo.insert_user(admin_id, UserRole::Admin);

        let backup_service = StubBackupService;
        let app_service =
            BackupApplicationServiceImpl::new(user_repo, backup_service, test_pool().await);

        let result = app_service
            .export_backup_secured(
                admin_id,
                unique_db_path().as_path(),
                std::path::Path::new("/tmp/backup.hvb"),
                &secrecy::SecretString::new("recovery phrase".to_string().into()),
            )
            .await;

        assert!(result.is_ok(), "export should succeed: {:?}", result.err());
    }

    #[tokio::test]
    async fn non_admin_cannot_export_backup() {
        let user_repo = StubUserRepo::default();
        let user_id = Uuid::new_v4();
        user_repo.insert_user(user_id, UserRole::User);

        let backup_service = StubBackupService;
        let app_service =
            BackupApplicationServiceImpl::new(user_repo, backup_service, test_pool().await);

        let result = app_service
            .export_backup_secured(
                user_id,
                unique_db_path().as_path(),
                std::path::Path::new("/tmp/backup.hvb"),
                &secrecy::SecretString::new("recovery phrase".to_string().into()),
            )
            .await;

        assert!(matches!(result, Err(AppError::Authorization(_))));
    }

    #[tokio::test]
    async fn admin_can_restore_backup() {
        let user_repo = StubUserRepo::default();
        let admin_id = Uuid::new_v4();
        user_repo.insert_user(admin_id, UserRole::Admin);

        let backup_service = StubBackupService;
        let app_service =
            BackupApplicationServiceImpl::new(user_repo, backup_service, test_pool().await);

        let result = app_service
            .restore_backup_secured(
                admin_id,
                std::path::Path::new("/tmp/backup.hvb"),
                &secrecy::SecretString::new("recovery phrase".to_string().into()),
                std::path::Path::new("/tmp/db_restored.db"),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn non_admin_cannot_restore_backup() {
        let user_repo = StubUserRepo::default();
        let user_id = Uuid::new_v4();
        user_repo.insert_user(user_id, UserRole::User);

        let backup_service = StubBackupService;
        let app_service =
            BackupApplicationServiceImpl::new(user_repo, backup_service, test_pool().await);

        let result = app_service
            .restore_backup_secured(
                user_id,
                std::path::Path::new("/tmp/backup.hvb"),
                &secrecy::SecretString::new("recovery phrase".to_string().into()),
                std::path::Path::new("/tmp/db_restored.db"),
            )
            .await;

        assert!(matches!(result, Err(AppError::Authorization(_))));
    }

    #[tokio::test]
    async fn missing_user_returns_not_found() {
        let user_repo = StubUserRepo::default();
        let missing_id = Uuid::new_v4();

        let backup_service = StubBackupService;
        let app_service =
            BackupApplicationServiceImpl::new(user_repo, backup_service, test_pool().await);

        let result = app_service
            .export_backup_secured(
                missing_id,
                unique_db_path().as_path(),
                std::path::Path::new("/tmp/backup.hvb"),
                &secrecy::SecretString::new("recovery phrase".to_string().into()),
            )
            .await;

        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn admin_can_export_rotation_backup_ticket() {
        let user_repo = StubUserRepo::default();
        let admin_id = Uuid::new_v4();
        user_repo.insert_user(admin_id, UserRole::Admin);

        let backup_service = StubBackupService;
        let app_service =
            BackupApplicationServiceImpl::new(user_repo, backup_service, test_pool().await);

        let result = app_service
            .export_rotation_backup_secured(
                admin_id,
                unique_db_path().as_path(),
                std::path::Path::new("/tmp/backup.hvb"),
            )
            .await;

        assert!(result.is_ok());
        let ticket = result.expect("rotation ticket should be returned");
        assert_eq!(ticket.backup_file_path, "/tmp/backup.hvb");
        assert_eq!(ticket.metadata_sha256_hex, "abc123");
    }

    #[tokio::test]
    async fn admin_can_restore_rotation_backup_ticket() {
        let user_repo = StubUserRepo::default();
        let admin_id = Uuid::new_v4();
        user_repo.insert_user(admin_id, UserRole::Admin);

        let backup_service = StubBackupService;
        let app_service =
            BackupApplicationServiceImpl::new(user_repo, backup_service, test_pool().await);

        let ticket = app_service
            .export_rotation_backup_secured(
                admin_id,
                unique_db_path().as_path(),
                std::path::Path::new("/tmp/backup.hvb"),
            )
            .await
            .expect("rotation backup export should succeed");

        let restore_result = app_service
            .restore_rotation_backup_secured(
                admin_id,
                &ticket,
                std::path::Path::new("/tmp/db_restored.db"),
            )
            .await;

        assert!(restore_result.is_ok());
    }

    /// Guards against reading the `.db` file directly: in WAL mode the recent rows only
    /// live in the `-wal` sidecar, so a raw copy would silently lose them.
    #[tokio::test]
    async fn snapshot_captures_rows_still_held_in_the_wal() {
        let db_path = unique_db_path();
        let pool = match sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
            .await
        {
            Ok(pool) => pool,
            Err(_) => return,
        };

        if sqlx::raw_sql(sqlx::AssertSqlSafe("PRAGMA journal_mode=WAL".to_string()))
            .execute(&pool)
            .await
            .is_err()
        {
            return;
        }
        if sqlx::query("CREATE TABLE probe (value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .is_err()
            || sqlx::query("INSERT INTO probe VALUES ('written-to-wal')")
                .execute(&pool)
                .await
                .is_err()
        {
            return;
        }

        let user_repo = StubUserRepo::default();
        let admin_id = Uuid::new_v4();
        user_repo.insert_user(admin_id, UserRole::Admin);
        let app_service = BackupApplicationServiceImpl::new(user_repo, StubBackupService, pool);

        let snapshot = match app_service.snapshot_database(db_path.as_path()).await {
            Ok(value) => value,
            Err(err) => panic!("snapshot should succeed: {err:?}"),
        };

        let snapshot_bytes = std::fs::read(snapshot.path.as_path()).unwrap_or_default();
        let raw_bytes = std::fs::read(db_path.as_path()).unwrap_or_default();

        assert!(
            snapshot_bytes
                .windows(14)
                .any(|window| window == b"written-to-wal"),
            "the snapshot must contain rows that are still in the WAL"
        );
        assert!(
            !raw_bytes.windows(14).any(|w| w == b"written-to-wal"),
            "precondition: the raw .db file does not hold the row yet"
        );

        let _ = std::fs::remove_file(&db_path);
    }
}

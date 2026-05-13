use std::path::Path;

use secrecy::SecretString;
use tracing::warn;
use uuid::Uuid;

use crate::errors::AppError;
use crate::repositories::user_repository::UserRepository;
use crate::services::access_control::{check_permission, Action, Resource};
use crate::services::backup_service::{BackupMetadata, BackupService};

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
}

pub struct BackupApplicationServiceImpl<TUserRepo, TBackupSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TBackupSvc: BackupService + Send + Sync,
{
    user_repo: TUserRepo,
    backup_service: TBackupSvc,
}

impl<TUserRepo, TBackupSvc> BackupApplicationServiceImpl<TUserRepo, TBackupSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TBackupSvc: BackupService + Send + Sync,
{
    pub fn new(user_repo: TUserRepo, backup_service: TBackupSvc) -> Self {
        Self {
            user_repo,
            backup_service,
        }
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

        self.backup_service.export_hvb_with_recovery_key(
            sqlite_db_path,
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
        fn export_backup(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<BackupMetadata, AppError> {
            Ok(BackupMetadata {
                sha256_hex: "ghi789".to_string(),
                plaintext_size: 512,
            })
        }
        fn import_backup(
            &self,
            _: &std::path::Path,
            _: &std::path::Path,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<BackupMetadata, AppError> {
            Ok(BackupMetadata {
                sha256_hex: "jkl012".to_string(),
                plaintext_size: 4096,
            })
        }
    }

    #[tokio::test]
    async fn admin_can_export_backup() {
        let user_repo = StubUserRepo::default();
        let admin_id = Uuid::new_v4();
        user_repo.insert_user(admin_id, UserRole::Admin);

        let backup_service = StubBackupService;
        let app_service = BackupApplicationServiceImpl::new(user_repo, backup_service);

        let result = app_service
            .export_backup_secured(
                admin_id,
                std::path::Path::new("/tmp/db.db"),
                std::path::Path::new("/tmp/backup.hvb"),
                &secrecy::SecretString::new("recovery phrase".to_string().into()),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn non_admin_cannot_export_backup() {
        let user_repo = StubUserRepo::default();
        let user_id = Uuid::new_v4();
        user_repo.insert_user(user_id, UserRole::User);

        let backup_service = StubBackupService;
        let app_service = BackupApplicationServiceImpl::new(user_repo, backup_service);

        let result = app_service
            .export_backup_secured(
                user_id,
                std::path::Path::new("/tmp/db.db"),
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
        let app_service = BackupApplicationServiceImpl::new(user_repo, backup_service);

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
        let app_service = BackupApplicationServiceImpl::new(user_repo, backup_service);

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
        let app_service = BackupApplicationServiceImpl::new(user_repo, backup_service);

        let result = app_service
            .export_backup_secured(
                missing_id,
                std::path::Path::new("/tmp/db.db"),
                std::path::Path::new("/tmp/backup.hvb"),
                &secrecy::SecretString::new("recovery phrase".to_string().into()),
            )
            .await;

        assert!(matches!(result, Err(AppError::NotFound(_))));
    }
}

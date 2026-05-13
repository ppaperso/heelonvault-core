#![allow(clippy::disallowed_methods)]

/// Shared test stubs for integration tests.
///
/// Usage: add `#[path = "../common/mod.rs"] mod common;` at the top of a test file,
/// or use `mod common;` when the test file lives adjacent to this directory.
use heelonvault_rust::errors::AppError;
use heelonvault_rust::models::{User, UserRole};
use std::sync::Arc;
use uuid::Uuid;

// ─── StubUserRepo ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct StubUserRepo {
    users: Arc<std::sync::Mutex<std::collections::HashMap<Uuid, User>>>,
}

impl Default for StubUserRepo {
    fn default() -> Self {
        Self {
            users: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }
}

impl StubUserRepo {
    pub fn add_user(&self, id: Uuid, role: UserRole) {
        self.users.lock().unwrap().insert(
            id,
            User {
                id,
                username: format!("user_{}", id),
                role,
                email: None,
                display_name: None,
                preferred_language: "en".to_string(),
                show_passwords_in_edit: false,
                updated_at: None,
            },
        );
    }
}

impl heelonvault_rust::repositories::user_repository::UserRepository for StubUserRepo {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        Ok(self.users.lock().unwrap().get(&id).cloned())
    }

    async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .find(|u| u.username == username)
            .cloned())
    }

    async fn resolve_username_for_login_identifier(
        &self,
        id: &str,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .find(|u| u.username == id)
            .map(|u| u.username.clone()))
    }

    async fn list_all(&self) -> Result<Vec<User>, AppError> {
        Ok(self.users.lock().unwrap().values().cloned().collect())
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

// ─── StubBackupService ───────────────────────────────────────────────────────

#[derive(Clone)]
pub struct StubBackupService;

impl heelonvault_rust::services::backup_service::BackupService for StubBackupService {
    fn generate_recovery_key(
        &self,
    ) -> Result<heelonvault_rust::services::backup_service::RecoveryKeyBundle, AppError> {
        Ok(heelonvault_rust::services::backup_service::RecoveryKeyBundle {
            recovery_phrase: secrecy::SecretString::new(
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
                    .to_string()
                    .into(),
            ),
        })
    }

    fn export_hvb_with_recovery_key(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &secrecy::SecretString,
    ) -> Result<heelonvault_rust::services::backup_service::BackupMetadata, AppError> {
        Ok(heelonvault_rust::services::backup_service::BackupMetadata {
            sha256_hex: "abcd1234".to_string(),
            plaintext_size: 1024,
        })
    }

    fn import_hvb_with_recovery_key(
        &self,
        _: &std::path::Path,
        _: &secrecy::SecretString,
        _: &std::path::Path,
    ) -> Result<heelonvault_rust::services::backup_service::BackupMetadata, AppError> {
        Ok(heelonvault_rust::services::backup_service::BackupMetadata {
            sha256_hex: "abcd1234".to_string(),
            plaintext_size: 1024,
        })
    }

    fn export_backup(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: secrecy::SecretBox<Vec<u8>>,
    ) -> Result<heelonvault_rust::services::backup_service::BackupMetadata, AppError> {
        Ok(heelonvault_rust::services::backup_service::BackupMetadata {
            sha256_hex: "abcd1234".to_string(),
            plaintext_size: 1024,
        })
    }

    fn import_backup(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: secrecy::SecretBox<Vec<u8>>,
    ) -> Result<heelonvault_rust::services::backup_service::BackupMetadata, AppError> {
        Ok(heelonvault_rust::services::backup_service::BackupMetadata {
            sha256_hex: "abcd1234".to_string(),
            plaintext_size: 1024,
        })
    }
}

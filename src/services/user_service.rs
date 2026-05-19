use secrecy::{ExposeSecret, SecretBox};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use uuid::Uuid;

use crate::errors::{AccessDeniedReason, AppError};
use crate::models::AccessibleVault;
use crate::models::User;
use crate::repositories::user_repository::UserRepository;
use crate::repositories::vault_repository::VaultRepository;
use crate::services::auth_service::AuthService;
use crate::services::backup_service::{BackupService, BackupServiceImpl};
use crate::services::crypto_service::{CryptoService, EncryptedPayload, NONCE_LEN};
use crate::services::vault_service::VaultKeyEnvelopeRepository;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationValidationMode {
    VaultOpenOnly,
    VaultAndSampleSecret,
}

#[derive(Debug, Clone)]
pub struct MasterKeyRotationPolicy {
    pub require_backup: bool,
    pub keep_backup_on_success: bool,
    pub validation_mode: RotationValidationMode,
    pub max_secrets_validate_per_vault: usize,
}

impl Default for MasterKeyRotationPolicy {
    fn default() -> Self {
        Self {
            require_backup: true,
            keep_backup_on_success: false,
            validation_mode: RotationValidationMode::VaultAndSampleSecret,
            max_secrets_validate_per_vault: 1,
        }
    }
}

#[derive(Debug)]
pub struct MasterKeyRotationRequest {
    pub user_id: Uuid,
    pub actor_id: Uuid,
    pub sqlite_db_path: PathBuf,
    pub backup_file_path: Option<PathBuf>,
    pub current_password: SecretBox<Vec<u8>>,
    pub new_password: SecretBox<Vec<u8>>,
    pub policy: MasterKeyRotationPolicy,
}

#[derive(Debug, Clone)]
pub struct MasterKeyRotationReport {
    pub rotation_id: Uuid,
    pub backup_path: Option<String>,
    pub scanned_vaults: usize,
    pub owner_vaults_rewrapped: usize,
    pub shared_vaults_rewrapped: usize,
    pub sample_secrets_validated: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug)]
pub struct UserProfileUpdate {
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub preferred_language: Option<String>,
    pub show_passwords_in_edit: Option<bool>,
    pub current_password: Option<SecretBox<Vec<u8>>>,
}

#[trait_variant::make(UserService: Send)]
pub trait LocalUserService {
    async fn get_user_profile(&self, user_id: Uuid) -> Result<User, AppError>;
    async fn get_user_profile_by_username(&self, username: &str) -> Result<User, AppError>;
    async fn resolve_username_for_login_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<String>, AppError>;
    async fn update_user_profile(
        &self,
        user_id: Uuid,
        update: UserProfileUpdate,
    ) -> Result<User, AppError>;
    async fn update_show_passwords_in_edit(
        &self,
        user_id: Uuid,
        show_passwords_in_edit: bool,
    ) -> Result<User, AppError>;
    async fn change_master_password(
        &self,
        user_id: Uuid,
        current_password: SecretBox<Vec<u8>>,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn rotate_master_key_hardened(
        &self,
        request: MasterKeyRotationRequest,
    ) -> Result<MasterKeyRotationReport, AppError>;
}

pub struct UserServiceImpl<TUserRepo, TVaultRepo, TEnvelopeRepo, TAuth, TCrypto>
where
    TUserRepo: UserRepository + Send + Sync,
    TVaultRepo: VaultRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    user_repo: TUserRepo,
    vault_repo: TVaultRepo,
    envelope_repo: TEnvelopeRepo,
    auth_service: Arc<TAuth>,
    crypto_service: TCrypto,
    rotation_in_progress: AtomicBool,
}

impl<TUserRepo, TVaultRepo, TEnvelopeRepo, TAuth, TCrypto>
    UserServiceImpl<TUserRepo, TVaultRepo, TEnvelopeRepo, TAuth, TCrypto>
where
    TUserRepo: UserRepository + Send + Sync,
    TVaultRepo: VaultRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    pub fn new(
        user_repo: TUserRepo,
        vault_repo: TVaultRepo,
        envelope_repo: TEnvelopeRepo,
        auth_service: Arc<TAuth>,
        crypto_service: TCrypto,
    ) -> Self {
        Self {
            user_repo,
            vault_repo,
            envelope_repo,
            auth_service,
            crypto_service,
            rotation_in_progress: AtomicBool::new(false),
        }
    }

    async fn validate_accessible_vaults_with_master_key(
        &self,
        user_id: Uuid,
        master_key: &SecretBox<Vec<u8>>,
    ) -> Result<(usize, usize, usize), AppError> {
        let accessible_vaults = self.vault_repo.get_accessible_vaults(user_id).await?;
        let owner_count = accessible_vaults
            .iter()
            .filter(|access| access.vault.owner_user_id == user_id)
            .count();
        let shared_count = accessible_vaults.len().saturating_sub(owner_count);

        self.decrypt_accessible_vault_envelopes(accessible_vaults, user_id, master_key)
            .await?;

        Ok((owner_count + shared_count, owner_count, shared_count))
    }

    async fn decrypt_accessible_vault_envelopes(
        &self,
        accessible_vaults: Vec<AccessibleVault>,
        user_id: Uuid,
        master_key: &SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        for access in accessible_vaults {
            let envelope = if access.vault.owner_user_id == user_id {
                self.envelope_repo
                    .get_vault_key_envelope(access.vault.id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Storage("missing owner vault key envelope".to_string())
                    })?
            } else {
                self.vault_repo
                    .get_key_share(access.vault.id, user_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Storage("missing shared vault key envelope".to_string())
                    })?
            };

            let payload = Self::deserialize_envelope(&envelope)?;
            let _ = self.crypto_service.decrypt(&payload, master_key).await?;
        }

        Ok(())
    }

    fn serialize_envelope(payload: &EncryptedPayload) -> SecretBox<Vec<u8>> {
        let mut bytes = Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
        bytes.extend_from_slice(&payload.nonce);
        bytes.extend_from_slice(payload.ciphertext.expose_secret().as_slice());
        SecretBox::new(Box::new(bytes))
    }

    fn deserialize_envelope(bytes: &SecretBox<Vec<u8>>) -> Result<EncryptedPayload, AppError> {
        if bytes.expose_secret().len() < NONCE_LEN {
            return Err(AppError::Storage(
                "vault key envelope is invalid".to_string(),
            ));
        }

        let mut nonce = [0_u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes.expose_secret()[0..NONCE_LEN]);
        let ciphertext = bytes.expose_secret()[NONCE_LEN..].to_vec();

        Ok(EncryptedPayload {
            nonce,
            ciphertext: SecretBox::new(Box::new(ciphertext)),
        })
    }
}

impl<TUserRepo, TVaultRepo, TEnvelopeRepo, TAuth, TCrypto> UserService
    for UserServiceImpl<TUserRepo, TVaultRepo, TEnvelopeRepo, TAuth, TCrypto>
where
    TUserRepo: UserRepository + Send + Sync,
    TVaultRepo: VaultRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    async fn get_user_profile(&self, user_id: Uuid) -> Result<User, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        Ok(user)
    }

    async fn get_user_profile_by_username(&self, username: &str) -> Result<User, AppError> {
        let user = self
            .user_repo
            .get_by_username(username)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        Ok(user)
    }

    async fn resolve_username_for_login_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<String>, AppError> {
        self.user_repo
            .resolve_username_for_login_identifier(identifier)
            .await
    }

    async fn update_user_profile(
        &self,
        user_id: Uuid,
        update: UserProfileUpdate,
    ) -> Result<User, AppError> {
        let current_user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;

        let next_email = update
            .email
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let next_display_name = update
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let next_preferred_language = update
            .preferred_language
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let email_changed = next_email != current_user.email;
        if email_changed {
            let current_password = update.current_password.ok_or({
                AppError::Authorization(AccessDeniedReason::PasswordRequiredForChange)
            })?;

            let password_ok = self
                .auth_service
                .verify_password(current_user.username.as_str(), current_password)
                .await?;
            if !password_ok {
                warn!(user_id = %user_id, "profile update denied: wrong current password for email change");
                return Err(AppError::Authorization(
                    AccessDeniedReason::InvalidCredentials,
                ));
            }
        }

        self.user_repo
            .update_user_profile(
                user_id,
                next_email.as_deref(),
                next_display_name.as_deref(),
                next_preferred_language.as_deref(),
                update.show_passwords_in_edit,
            )
            .await?;

        info!(user_id = %user_id, email_changed = email_changed, "user profile updated");

        let updated_user =
            self.user_repo.get_by_id(user_id).await?.ok_or_else(|| {
                AppError::NotFound("user not found after profile update".to_string())
            })?;

        Ok(updated_user)
    }

    async fn change_master_password(
        &self,
        user_id: Uuid,
        current_password: SecretBox<Vec<u8>>,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;

        if new_password.expose_secret().len() < 10 {
            return Err(AppError::Validation(
                "new password must contain at least 10 characters".to_string(),
            ));
        }

        let current_password_bytes = current_password.expose_secret().clone();
        let new_password_bytes = new_password.expose_secret().clone();

        let old_master_key = self
            .auth_service
            .derive_key_if_valid(
                user.username.as_str(),
                SecretBox::new(Box::new(current_password_bytes)),
            )
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;

        self.auth_service
            .change_password(user.username.as_str(), current_password, new_password)
            .await?;

        let password_envelope = self
            .auth_service
            .get_password_envelope(user.username.as_str())
            .await?;

        self.user_repo
            .update_password_envelope(user_id, password_envelope)
            .await?;

        let new_master_key = self
            .auth_service
            .derive_key_if_valid(
                user.username.as_str(),
                SecretBox::new(Box::new(new_password_bytes)),
            )
            .await?
            .ok_or(AppError::Internal)?;

        // Re-wrap all accessible vault key envelopes so existing secrets remain decryptable
        // with the new user master key after password rotation.
        let accessible_vaults = self.vault_repo.get_accessible_vaults(user_id).await?;
        for access in accessible_vaults {
            let envelope = if access.vault.owner_user_id == user_id {
                self.envelope_repo
                    .get_vault_key_envelope(access.vault.id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Storage("missing owner vault key envelope".to_string())
                    })?
            } else {
                self.vault_repo
                    .get_key_share(access.vault.id, user_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Storage("missing shared vault key envelope".to_string())
                    })?
            };

            let payload = Self::deserialize_envelope(&envelope)?;
            let vault_key = self
                .crypto_service
                .decrypt(&payload, &old_master_key)
                .await?;
            let rewrapped_payload = self
                .crypto_service
                .encrypt(&vault_key, &new_master_key)
                .await?;
            let rewrapped_envelope = Self::serialize_envelope(&rewrapped_payload);

            if access.vault.owner_user_id == user_id {
                self.vault_repo
                    .update_vault_key_envelope(access.vault.id, rewrapped_envelope)
                    .await?;
            } else {
                self.vault_repo
                    .update_key_share_envelope(access.vault.id, user_id, rewrapped_envelope)
                    .await?;
            }
        }

        info!(user_id = %user_id, "master password changed");
        Ok(())
    }

    async fn rotate_master_key_hardened(
        &self,
        request: MasterKeyRotationRequest,
    ) -> Result<MasterKeyRotationReport, AppError> {
        let started = std::time::Instant::now();
        let rotation_id = Uuid::new_v4();

        if self
            .rotation_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AppError::Conflict(
                "master key rotation already in progress".to_string(),
            ));
        }

        struct RotationResetGuard<'a> {
            flag: &'a AtomicBool,
        }

        impl<'a> Drop for RotationResetGuard<'a> {
            fn drop(&mut self) {
                self.flag.store(false, Ordering::SeqCst);
            }
        }

        let _rotation_reset = RotationResetGuard {
            flag: &self.rotation_in_progress,
        };

        let user = self
            .user_repo
            .get_by_id(request.user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;

        let current_password_bytes = request.current_password.expose_secret().clone();
        let new_password_bytes = request.new_password.expose_secret().clone();

        let old_master_key = self
            .auth_service
            .derive_key_if_valid(
                user.username.as_str(),
                SecretBox::new(Box::new(current_password_bytes.clone())),
            )
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;

        let (scanned_vaults, owner_vaults_rewrapped, shared_vaults_rewrapped) = self
            .validate_accessible_vaults_with_master_key(request.user_id, &old_master_key)
            .await?;

        let backup_service = BackupServiceImpl::new();
        let mut backup_path_for_report: Option<String> = None;
        let mut backup_recovery_phrase: Option<secrecy::SecretString> = None;

        if request.policy.require_backup {
            let backup_path = if let Some(path) = request.backup_file_path.clone() {
                path
            } else {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_err| AppError::Internal)?
                    .as_secs();
                request
                    .sqlite_db_path
                    .with_extension(format!("master-rotation-{rotation_id}-{stamp}.hvb"))
            };

            let recovery = backup_service.generate_recovery_key()?;
            backup_service.export_hvb_with_recovery_key(
                request.sqlite_db_path.as_path(),
                backup_path.as_path(),
                &recovery.recovery_phrase,
            )?;

            backup_path_for_report = Some(backup_path.to_string_lossy().to_string());
            backup_recovery_phrase = Some(recovery.recovery_phrase);
        }

        let mutation_result = UserService::change_master_password(
            self,
            request.user_id,
            SecretBox::new(Box::new(current_password_bytes)),
            SecretBox::new(Box::new(new_password_bytes.clone())),
        )
        .await;

        if let Err(error) = mutation_result {
            return Err(error);
        }

        let new_master_key = self
            .auth_service
            .derive_key_if_valid(
                user.username.as_str(),
                SecretBox::new(Box::new(new_password_bytes)),
            )
            .await?
            .ok_or(AppError::Internal)?;

        let post_validation = self
            .validate_accessible_vaults_with_master_key(request.user_id, &new_master_key)
            .await;

        if let Err(validation_error) = post_validation {
            if let (Some(backup_path), Some(recovery_phrase)) = (
                backup_path_for_report.clone(),
                backup_recovery_phrase,
            ) {
                let restore_result = backup_service.import_hvb_with_recovery_key(
                    PathBuf::from(backup_path.as_str()).as_path(),
                    &recovery_phrase,
                    request.sqlite_db_path.as_path(),
                );

                if restore_result.is_ok() {
                    if let Some(restored_password_envelope) = self
                        .user_repo
                        .get_password_envelope_by_user_id(request.user_id)
                        .await?
                    {
                        self.auth_service
                            .upsert_password_envelope(
                                user.username.as_str(),
                                restored_password_envelope,
                            )
                            .await?;
                    }
                    return Err(AppError::Storage(format!(
                        "master key rotation failed, backup restored automatically: {validation_error}"
                    )));
                }
            }

            return Err(validation_error);
        }

        let sample_secrets_validated = match request.policy.validation_mode {
            RotationValidationMode::VaultOpenOnly => 0,
            RotationValidationMode::VaultAndSampleSecret => 0,
        };
        let _ = request.policy.max_secrets_validate_per_vault;
        let _ = request.policy.keep_backup_on_success;
        let _ = request.actor_id;

        Ok(MasterKeyRotationReport {
            rotation_id,
            backup_path: backup_path_for_report,
            scanned_vaults,
            owner_vaults_rewrapped,
            shared_vaults_rewrapped,
            sample_secrets_validated,
            elapsed_ms: started.elapsed().as_millis(),
        })
    }

    async fn update_show_passwords_in_edit(
        &self,
        user_id: Uuid,
        show_passwords_in_edit: bool,
    ) -> Result<User, AppError> {
        self.user_repo
            .update_show_passwords_in_edit(user_id, show_passwords_in_edit)
            .await?;

        let updated_user = self.user_repo.get_by_id(user_id).await?.ok_or_else(|| {
            AppError::NotFound("user not found after preference update".to_string())
        })?;

        Ok(updated_user)
    }
}

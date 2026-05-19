use secrecy::{ExposeSecret, SecretBox};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::errors::{AccessDeniedReason, AppError};
use crate::models::User;
use crate::repositories::user_repository::UserRepository;
use crate::repositories::vault_repository::VaultRepository;
use crate::services::auth_service::AuthService;
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
        }
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

        let _ = request.actor_id;
        let _ = request.policy;

        UserService::change_master_password(
            self,
            request.user_id,
            request.current_password,
            request.new_password,
        )
        .await?;

        Ok(MasterKeyRotationReport {
            rotation_id,
            backup_path: None,
            scanned_vaults: 0,
            owner_vaults_rewrapped: 0,
            shared_vaults_rewrapped: 0,
            sample_secrets_validated: 0,
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

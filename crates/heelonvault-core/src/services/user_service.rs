use secrecy::{ExposeSecret, SecretBox, SecretString};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tracing::{info, warn};
use uuid::Uuid;

use crate::errors::{AccessDeniedReason, AppError};
use crate::models::User;
use crate::repositories::secret_repository::SecretRepository;
use crate::repositories::user_repository::{UserKeyMaterialRepository, UserRepository};
use crate::repositories::vault_repository::{MasterKeyRotationRepository, VaultRepository};
use crate::services::auth_service::{AuthService, PasswordEnvelope, decode_password_envelope};
use crate::services::crypto_service::CryptoService;
use crate::services::rekey_service::{
    RekeyRepositories, migrate_to_account_key, rekey_legacy_account, replace_password,
};
use crate::services::vault_service::VaultKeyEnvelopeRepository;

fn password_text(password: &SecretBox<Vec<u8>>) -> Result<SecretString, AppError> {
    std::str::from_utf8(password.expose_secret().as_slice())
        .map(|text| SecretString::new(text.into()))
        .map_err(|_| AppError::Validation("password must be valid utf-8".to_string()))
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
    ) -> Result<SecretBox<Vec<u8>>, AppError>;
    /// Moves a single-account vault still on the legacy password envelope to an account key.
    /// Returns the key the session must use, or `None` when nothing had to change.
    async fn upgrade_legacy_credentials(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError>;
    /// Whether the account key is not sealed with the recovery phrase, so a backup could not be
    /// reopened with it.
    async fn account_needs_recovery_key(&self, user_id: Uuid) -> Result<bool, AppError>;
}

pub struct UserServiceImpl<TUserRepo, TVaultRepo, TEnvelopeRepo, TSecretRepo, TAuth, TCrypto>
where
    TUserRepo: UserRepository + UserKeyMaterialRepository + Send + Sync,
    TVaultRepo: VaultRepository + MasterKeyRotationRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TSecretRepo: SecretRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    user_repo: TUserRepo,
    vault_repo: TVaultRepo,
    envelope_repo: TEnvelopeRepo,
    secret_repo: TSecretRepo,
    auth_service: Arc<TAuth>,
    crypto_service: TCrypto,
}

impl<TUserRepo, TVaultRepo, TEnvelopeRepo, TSecretRepo, TAuth, TCrypto>
    UserServiceImpl<TUserRepo, TVaultRepo, TEnvelopeRepo, TSecretRepo, TAuth, TCrypto>
where
    TUserRepo: UserRepository + UserKeyMaterialRepository + Send + Sync,
    TVaultRepo: VaultRepository + MasterKeyRotationRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TSecretRepo: SecretRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    pub fn new(
        user_repo: TUserRepo,
        vault_repo: TVaultRepo,
        envelope_repo: TEnvelopeRepo,
        secret_repo: TSecretRepo,
        auth_service: Arc<TAuth>,
        crypto_service: TCrypto,
    ) -> Self {
        Self {
            user_repo,
            vault_repo,
            envelope_repo,
            secret_repo,
            auth_service,
            crypto_service,
        }
    }

    fn rekey_repositories(
        &self,
    ) -> RekeyRepositories<'_, TUserRepo, TVaultRepo, TEnvelopeRepo, TSecretRepo> {
        RekeyRepositories {
            users: &self.user_repo,
            vaults: &self.vault_repo,
            envelopes: &self.envelope_repo,
            secrets: &self.secret_repo,
        }
    }

    /// Only single-account vaults move to an account key (see `AppError::SingleAccountVault`).
    async fn is_single_account(&self) -> Result<bool, AppError> {
        Ok(self.user_repo.list_all().await?.len() == 1)
    }
}

impl<TUserRepo, TVaultRepo, TEnvelopeRepo, TSecretRepo, TAuth, TCrypto> UserService
    for UserServiceImpl<TUserRepo, TVaultRepo, TEnvelopeRepo, TSecretRepo, TAuth, TCrypto>
where
    TUserRepo: UserRepository + UserKeyMaterialRepository + Send + Sync,
    TVaultRepo: VaultRepository + MasterKeyRotationRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TSecretRepo: SecretRepository + Send + Sync,
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

    /// Returns the new master key so the running session can keep opening vaults.
    async fn change_master_password(
        &self,
        user_id: Uuid,
        current_password: SecretBox<Vec<u8>>,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;

        if new_password.expose_secret().len() < 16 {
            return Err(AppError::Validation(
                "new password must contain at least 16 characters".to_string(),
            ));
        }
        if bool::from(
            current_password
                .expose_secret()
                .as_slice()
                .ct_eq(new_password.expose_secret().as_slice()),
        ) {
            return Err(AppError::Validation(
                "new password must be different from current password".to_string(),
            ));
        }
        let new_password = password_text(&new_password)?;

        let current_key = self
            .auth_service
            .derive_key_if_valid(user.username.as_str(), current_password)
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;
        let stored = decode_password_envelope(
            &self
                .auth_service
                .get_password_envelope(user.username.as_str())
                .await?,
        )?;
        let single_account = self.is_single_account().await?;

        let repos = self.rekey_repositories();
        let crypto = &self.crypto_service;
        let outcome = match stored {
            PasswordEnvelope::AccountKey { .. } => {
                replace_password(repos, crypto, user_id, &current_key, &new_password).await?
            }
            PasswordEnvelope::Legacy { .. } if single_account => {
                migrate_to_account_key(repos, crypto, user_id, &current_key, &new_password, None)
                    .await?
            }
            PasswordEnvelope::Legacy { .. } => {
                rekey_legacy_account(repos, crypto, user_id, &current_key, &new_password).await?
            }
        };

        self.auth_service
            .upsert_password_envelope(user.username.as_str(), outcome.password_envelope)
            .await?;

        info!(
            user_id = %user_id,
            owner_vaults_rewrapped = outcome.report.owner_vaults_rewrapped,
            "master password changed"
        );
        Ok(outcome.session_key)
    }

    async fn upgrade_legacy_credentials(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        let stored =
            decode_password_envelope(&self.auth_service.get_password_envelope(username).await?)?;
        if stored.is_account_key() || !self.is_single_account().await? {
            return Ok(None);
        }

        let user = self
            .user_repo
            .get_by_username(username)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        let password_string = password_text(&password)?;
        let legacy_key = self
            .auth_service
            .derive_key_if_valid(username, password)
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;

        let outcome = migrate_to_account_key(
            self.rekey_repositories(),
            &self.crypto_service,
            user.id,
            &legacy_key,
            &password_string,
            None,
        )
        .await?;
        self.auth_service
            .upsert_password_envelope(username, outcome.password_envelope)
            .await?;

        info!(
            user_id = %user.id,
            recovery_key_available = outcome.report.recovery_key_available,
            "credentials migrated to an account key"
        );
        Ok(Some(outcome.session_key))
    }

    async fn account_needs_recovery_key(&self, user_id: Uuid) -> Result<bool, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        let stored = decode_password_envelope(
            &self
                .auth_service
                .get_password_envelope(user.username.as_str())
                .await?,
        )?;
        Ok(stored.is_account_key()
            && self
                .user_repo
                .get_recovery_key_envelope(user_id)
                .await?
                .is_none())
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

use secrecy::{ExposeSecret, SecretBox};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::errors::{AccessDeniedReason, AppError};
use crate::models::User;
use crate::repositories::user_repository::UserRepository;
use crate::services::auth_service::AuthService;

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
}

pub struct UserServiceImpl<TRepo, TAuth>
where
    TRepo: UserRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
{
    user_repo: TRepo,
    auth_service: Arc<TAuth>,
}

impl<TRepo, TAuth> UserServiceImpl<TRepo, TAuth>
where
    TRepo: UserRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
{
    pub fn new(user_repo: TRepo, auth_service: Arc<TAuth>) -> Self {
        Self {
            user_repo,
            auth_service,
        }
    }
}

impl<TRepo, TAuth> UserService for UserServiceImpl<TRepo, TAuth>
where
    TRepo: UserRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
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

        info!(user_id = %user_id, "master password changed");
        Ok(())
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

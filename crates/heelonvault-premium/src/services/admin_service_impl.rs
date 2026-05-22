use std::sync::Arc;

use secrecy::{ExposeSecret, SecretBox};
use tracing::info;
use uuid::Uuid;

use heelonvault_core::errors::AppError;
use heelonvault_core::models::{AuditAction, User, UserRole};
use heelonvault_core::repositories::user_repository::UserRepository;
use heelonvault_core::services::access_control::{check_permission, Action, Resource};
use heelonvault_core::services::admin_service::{AdminService, CreateUserResult};
use heelonvault_core::services::audit_log_service::AuditLogService;
use heelonvault_core::services::auth_service::AuthService;

pub struct AdminServiceImpl<TUserRepo, TAuth, TAuditSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    user_repo: TUserRepo,
    auth_service: Arc<TAuth>,
    audit_service: Arc<TAuditSvc>,
}

impl<TUserRepo, TAuth, TAuditSvc> AdminServiceImpl<TUserRepo, TAuth, TAuditSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    pub fn new(
        user_repo: TUserRepo,
        auth_service: Arc<TAuth>,
        audit_service: Arc<TAuditSvc>,
    ) -> Self {
        Self {
            user_repo,
            auth_service,
            audit_service,
        }
    }

    async fn require_admin(&self, actor_id: Uuid) -> Result<User, AppError> {
        let user = self
            .user_repo
            .get_by_id(actor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("actor user not found".to_string()))?;
        check_permission(&user, Action::AdminManageUsers, &Resource::Global)?;
        Ok(user)
    }

    /// Returns the number of admin accounts in the DB.
    async fn admin_count(&self) -> Result<usize, AppError> {
        let all = self.user_repo.list_all().await?;
        Ok(all
            .iter()
            .filter(|u| matches!(u.role, UserRole::Admin))
            .count())
    }
}

impl<TUserRepo, TAuth, TAuditSvc> AdminService for AdminServiceImpl<TUserRepo, TAuth, TAuditSvc>
where
    TUserRepo: UserRepository + Send + Sync,
    TAuth: AuthService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    async fn create_user(
        &self,
        actor_id: Uuid,
        username: &str,
        password: SecretBox<Vec<u8>>,
        role: UserRole,
    ) -> Result<CreateUserResult, AppError> {
        self.require_admin(actor_id).await?;

        let trimmed = username.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "username must not be empty".to_string(),
            ));
        }

        let password_bytes = password.expose_secret().clone();
        self.auth_service
            .create_user(trimmed, SecretBox::new(Box::new(password_bytes.clone())))
            .await?;

        let master_key = self
            .auth_service
            .derive_key_if_valid(trimmed, SecretBox::new(Box::new(password_bytes)))
            .await?
            .ok_or(AppError::Internal)?;

        let envelope = self.auth_service.get_password_envelope(trimmed).await?;
        let user_id = Uuid::new_v4();
        self.user_repo
            .create_user_db(user_id, trimmed, &role)
            .await?;
        self.user_repo
            .update_password_envelope(user_id, envelope)
            .await?;

        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::Internal)?;

        self.audit_service
            .record_event(
                Some(actor_id),
                AuditAction::UserCreated,
                Some("user"),
                Some(&user_id.to_string()),
                Some(&format!(
                    r#"{{"username":"{}","role":"{}"}}"#,
                    trimmed,
                    role.to_db_str()
                )),
            )
            .await?;

        info!(actor = %actor_id, new_user = %user_id, username = trimmed, "admin created user");
        Ok(CreateUserResult { user, master_key })
    }

    async fn delete_user(&self, actor_id: Uuid, target_user_id: Uuid) -> Result<(), AppError> {
        self.require_admin(actor_id).await?;

        let target = self
            .user_repo
            .get_by_id(target_user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("target user not found".to_string()))?;

        if matches!(target.role, UserRole::Admin) && self.admin_count().await? <= 1 {
            return Err(AppError::Validation(
                "cannot delete the last admin account".to_string(),
            ));
        }

        self.user_repo.delete_user(target_user_id).await?;

        self.audit_service
            .record_event(
                Some(actor_id),
                AuditAction::UserDeleted,
                Some("user"),
                Some(&target_user_id.to_string()),
                Some(&format!(r#"{{"username":"{}"}}"#, target.username)),
            )
            .await?;

        info!(actor = %actor_id, deleted_user = %target_user_id, "admin deleted user");
        Ok(())
    }

    async fn update_user_role(
        &self,
        actor_id: Uuid,
        target_user_id: Uuid,
        new_role: UserRole,
    ) -> Result<(), AppError> {
        self.require_admin(actor_id).await?;

        let target = self
            .user_repo
            .get_by_id(target_user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("target user not found".to_string()))?;

        if matches!(target.role, UserRole::Admin)
            && matches!(new_role, UserRole::User)
            && self.admin_count().await? <= 1
        {
            return Err(AppError::Validation(
                "cannot demote the last admin account".to_string(),
            ));
        }

        self.user_repo
            .update_user_role(target_user_id, &new_role)
            .await?;

        self.audit_service
            .record_event(
                Some(actor_id),
                AuditAction::UserRoleChanged,
                Some("user"),
                Some(&target_user_id.to_string()),
                Some(&format!(
                    r#"{{"old_role":"{}","new_role":"{}"}}"#,
                    target.role.to_db_str(),
                    new_role.to_db_str()
                )),
            )
            .await?;

        info!(actor = %actor_id, target = %target_user_id, role = new_role.to_db_str(), "admin changed user role");
        Ok(())
    }

    async fn list_all_users(&self, actor_id: Uuid) -> Result<Vec<User>, AppError> {
        self.require_admin(actor_id).await?;
        self.user_repo.list_all().await
    }

    async fn reset_user_password(
        &self,
        actor_id: Uuid,
        target_user_id: Uuid,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        self.require_admin(actor_id).await?;

        let target = self
            .user_repo
            .get_by_id(target_user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("target user not found".to_string()))?;

        let password_bytes = new_password.expose_secret().clone();

        self.auth_service
            .upsert_password_envelope(&target.username, {
                use heelonvault_core::services::auth_service::AuthServiceImpl;
                use heelonvault_core::services::crypto_service::CryptoServiceImpl;
                let tmp = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
                tmp.create_user(
                    &target.username,
                    SecretBox::new(Box::new(password_bytes.clone())),
                )
                .await?;
                tmp.get_password_envelope(&target.username).await?
            })
            .await?;

        let new_envelope = self
            .auth_service
            .get_password_envelope(&target.username)
            .await?;
        self.user_repo
            .update_password_envelope(target_user_id, new_envelope)
            .await?;

        let master_key = self
            .auth_service
            .derive_key_if_valid(&target.username, SecretBox::new(Box::new(password_bytes)))
            .await?
            .ok_or(AppError::Internal)?;

        self.audit_service
            .record_event(
                Some(actor_id),
                AuditAction::UserPasswordReset,
                Some("user"),
                Some(&target_user_id.to_string()),
                None,
            )
            .await?;

        info!(actor = %actor_id, target = %target_user_id, "admin reset user password");
        Ok(master_key)
    }
}

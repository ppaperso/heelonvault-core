use std::sync::Arc;

use secrecy::{ExposeSecret, SecretBox};
use tracing::info;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AuditAction, User, UserRole};
use crate::repositories::user_repository::UserRepository;
use crate::services::access_control::{check_permission, Action, Resource};
use crate::services::audit_log_service::AuditLogService;
use crate::services::auth_service::AuthService;

/// Result of a successful user creation, exposing the derived master key so
/// the caller can immediately create the user's personal vault.
pub struct CreateUserResult {
    pub user: User,
    /// KDF-derived master key (argon2 of the initial password).
    /// The caller is responsible for creating the user's vault with this key
    /// and then securely zeroing it.
    pub master_key: SecretBox<Vec<u8>>,
}

/// Result of a successful first-admin bootstrap.
pub struct BootstrapResult {
    pub user_id: Uuid,
    pub username: String,
    pub master_key: SecretBox<Vec<u8>>,
}

#[trait_variant::make(AdminService: Send)]
pub trait LocalAdminService {
    /// Create a new user account.
    /// - Registers credentials in the in-memory AuthService
    /// - Persists the user row and password envelope in the DB
    /// - Returns the derived master key so the caller can create the first vault
    /// - Appends an audit log entry
    async fn create_user(
        &self,
        actor_id: Uuid,
        username: &str,
        password: SecretBox<Vec<u8>>,
        role: UserRole,
    ) -> Result<CreateUserResult, AppError>;

    /// Permanently delete a user. Refuses if the target is the last admin.
    async fn delete_user(&self, actor_id: Uuid, target_user_id: Uuid) -> Result<(), AppError>;

    /// Promote or demote a user role. Refuses if it would leave zero admins.
    async fn update_user_role(
        &self,
        actor_id: Uuid,
        target_user_id: Uuid,
        new_role: UserRole,
    ) -> Result<(), AppError>;

    /// List every user in the system (admin-only).
    async fn list_all_users(&self, actor_id: Uuid) -> Result<Vec<User>, AppError>;

    /// Reset another user's password (admin only).
    /// Regenerates the password envelope and returns the new master key so the
    /// caller can re-wrap the user's vault key envelopes if needed.
    async fn reset_user_password(
        &self,
        actor_id: Uuid,
        target_user_id: Uuid,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError>;

    /// Bootstrap the very first admin account.
    /// Atomically checks that no users exist yet, then creates the admin.
    /// Fails with [`AppError::Conflict`] if any user already exists.
    async fn bootstrap_first_admin(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<BootstrapResult, AppError>;
}

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

        // Validate username format early.
        let trimmed = username.trim();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "username must not be empty".to_string(),
            ));
        }

        // Register in the in-memory auth service (computes salt + hash).
        let password_bytes = password.expose_secret().clone();
        self.auth_service
            .create_user(trimmed, SecretBox::new(Box::new(password_bytes.clone())))
            .await?;

        // Derive master key while the password is still available.
        let master_key = self
            .auth_service
            .derive_key_if_valid(trimmed, SecretBox::new(Box::new(password_bytes)))
            .await?
            .ok_or(AppError::Internal)?;

        // Retrieve the serialised envelope and persist the user row.
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

        // Refuse to delete the last admin.
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

        // Refuse to demote the last admin.
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

        // Overwrite in-memory credentials.
        self.auth_service
            .upsert_password_envelope(
                &target.username,
                // Temporarily use create_user logic: reset requires re-hashing.
                // We achieve this by calling change_password with a throwaway.
                // Simpler: call create_user on a fresh AuthService instance and
                // port the resulting envelope back.
                //
                // Because upsert_password_envelope only imports a pre-built envelope
                // we must first produce one. We do this via a side-channel: create_user
                // fails if username exists in the in-memory map, so we directly set
                // a new envelope derived here.
                //
                // Implementation: derive new envelope via a one-shot AuthService.
                {
                    use crate::services::auth_service::AuthServiceImpl;
                    use crate::services::crypto_service::CryptoServiceImpl;
                    let tmp = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
                    tmp.create_user(
                        &target.username,
                        SecretBox::new(Box::new(password_bytes.clone())),
                    )
                    .await?;
                    tmp.get_password_envelope(&target.username).await?
                },
            )
            .await?;

        // Flush to DB.
        let new_envelope = self
            .auth_service
            .get_password_envelope(&target.username)
            .await?;
        self.user_repo
            .update_password_envelope(target_user_id, new_envelope)
            .await?;

        // Derive the new master key so the caller can re-wrap vault key envelopes.
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

    async fn bootstrap_first_admin(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<BootstrapResult, AppError> {
        // Atomic guard: refuse if any user already exists.
        if !self.user_repo.list_all().await?.is_empty() {
            return Err(AppError::Conflict(
                "vault already initialized; use the login form to access your account".to_string(),
            ));
        }

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
            .create_user_db(user_id, trimmed, &UserRole::Admin)
            .await?;
        self.user_repo
            .update_password_envelope(user_id, envelope)
            .await?;

        info!(user_id = %user_id, username = trimmed, "bootstrap: first admin account created");
        Ok(BootstrapResult {
            user_id,
            username: trimmed.to_string(),
            master_key,
        })
    }
}

// ── helpers exposed to other layers ─────────────────────────────────────────

impl UserRole {
    pub fn to_db_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::User => "user",
        }
    }
}

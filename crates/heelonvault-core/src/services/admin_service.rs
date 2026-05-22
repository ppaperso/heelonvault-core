use secrecy::{ExposeSecret, SecretBox};
use tracing::info;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{User, UserRole};
use crate::repositories::user_repository::UserRepository;
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
}

// ── bootstrap (always compiled — runs before any admin service is wired) ────────

/// Bootstrap the very first admin account.
/// Atomically checks that no users exist yet, then creates the admin.
/// Fails with [`AppError::Conflict`] if any user already exists.
///
/// This is a free function (not a trait method) so it is available in Community
/// builds where `AdminServiceImpl` is not compiled.
pub async fn bootstrap_first_admin(
    user_repo: &impl UserRepository,
    auth_service: &impl AuthService,
    username: &str,
    password: SecretBox<Vec<u8>>,
) -> Result<BootstrapResult, AppError> {
    // Atomic guard: refuse if any user already exists.
    if !user_repo.list_all().await?.is_empty() {
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
    auth_service
        .create_user(trimmed, SecretBox::new(Box::new(password_bytes.clone())))
        .await?;

    let master_key = auth_service
        .derive_key_if_valid(trimmed, SecretBox::new(Box::new(password_bytes)))
        .await?
        .ok_or(AppError::Internal)?;

    let envelope = auth_service.get_password_envelope(trimmed).await?;
    let user_id = Uuid::new_v4();
    user_repo
        .create_user_db(user_id, trimmed, &UserRole::Admin)
        .await?;
    user_repo
        .update_password_envelope(user_id, envelope)
        .await?;

    info!(user_id = %user_id, username = trimmed, "bootstrap: first admin account created");
    Ok(BootstrapResult {
        user_id,
        username: trimmed.to_string(),
        master_key,
    })
}

// ── community stub ──────────────────────────────────────────────────────────

/// Community build stub: all admin operations return [`AppError::FeatureNotAvailable`].
/// Multi-user management requires the `premium` feature.
pub struct CommunityAdminService;

impl AdminService for CommunityAdminService {
    async fn create_user(
        &self,
        _actor_id: Uuid,
        _username: &str,
        _password: SecretBox<Vec<u8>>,
        _role: UserRole,
    ) -> Result<CreateUserResult, AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-user-management",
        ))
    }

    async fn delete_user(&self, _actor_id: Uuid, _target_user_id: Uuid) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-user-management",
        ))
    }

    async fn update_user_role(
        &self,
        _actor_id: Uuid,
        _target_user_id: Uuid,
        _new_role: UserRole,
    ) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-user-management",
        ))
    }

    async fn list_all_users(&self, _actor_id: Uuid) -> Result<Vec<User>, AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-user-management",
        ))
    }

    async fn reset_user_password(
        &self,
        _actor_id: Uuid,
        _target_user_id: Uuid,
        _new_password: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-user-management",
        ))
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

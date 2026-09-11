use secrecy::{ExposeSecret, SecretBox, SecretString};
use tracing::info;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{User, UserRole};
use crate::repositories::user_repository::{UserKeyMaterialRepository, UserRepository};
use crate::services::account_key::{generate_account_key, seal_with_recovery_phrase};
use crate::services::auth_service::{AuthService, encode_password_envelope, wrap_account_key};
use crate::services::crypto_service::CryptoService;

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
    pub recovery_phrase: SecretString,
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

/// Bootstrap the very first admin account with the recovery phrase that was already
/// generated and shown to the user. The caller must pass the exact phrase the user
/// wrote down — never a freshly generated one, or backups become unrecoverable.
///
/// Atomically checks that no users exist yet. Fails with [`AppError::Conflict`] if any
/// user already exists.
///
/// This is a free function (not a trait method) so it is available in Community
/// builds where `AdminServiceImpl` is not compiled.
pub async fn bootstrap_first_admin_with_recovery(
    user_repo: &(impl UserRepository + UserKeyMaterialRepository),
    auth_service: &impl AuthService,
    backup_service: &impl crate::services::backup_service::BackupService,
    crypto_service: &(impl CryptoService + Sync),
    username: &str,
    password: SecretBox<Vec<u8>>,
    recovery_phrase: SecretString,
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
    let password_text = std::str::from_utf8(password.expose_secret().as_slice())
        .map(|text| SecretString::new(text.into()))
        .map_err(|_| AppError::Validation("password must be valid utf-8".to_string()))?;

    // A single account from the start: its vault keys go under a random account key, stored
    // only encrypted, under the password and under the recovery phrase.
    let account_key = generate_account_key()?;
    let envelope = encode_password_envelope(
        &wrap_account_key(crypto_service, &password_text, &account_key).await?,
    );
    auth_service
        .upsert_password_envelope(
            trimmed,
            SecretBox::new(Box::new(envelope.expose_secret().clone())),
        )
        .await?;

    let user_id = Uuid::new_v4();
    user_repo
        .create_user_db(user_id, trimmed, &UserRole::Admin)
        .await?;
    user_repo
        .update_password_envelope(user_id, envelope)
        .await?;
    user_repo
        .set_recovery_key_envelope(
            user_id,
            seal_with_recovery_phrase(crypto_service, &recovery_phrase, &account_key).await?,
        )
        .await?;

    // Verifier used to gate exports; the phrase must be re-typed by the user.
    let verifier = backup_service.build_recovery_verifier(&recovery_phrase)?;
    user_repo.set_recovery_verifier(user_id, verifier).await?;

    info!(user_id = %user_id, username = trimmed, "bootstrap: first admin account created with an account key");
    Ok(BootstrapResult {
        user_id,
        username: trimmed.to_string(),
        master_key: account_key,
        recovery_phrase,
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

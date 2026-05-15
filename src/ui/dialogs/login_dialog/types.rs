use std::sync::Arc;

use secrecy::SecretBox;
use uuid::Uuid;

use crate::errors::AppError;
use crate::services::admin_service::BootstrapResult;

pub struct AuthenticatedSession {
    pub user_id: Uuid,
    pub username: String,
    pub identity_label: String,
    pub master_key: SecretBox<Vec<u8>>,
}

pub struct BootstrapServicesContext {
    pub generate_recovery_key: Arc<dyn Fn() -> Result<String, AppError> + Send + Sync>,
    pub do_bootstrap:
        Arc<dyn Fn(String, Vec<u8>) -> Result<BootstrapResult, AppError> + Send + Sync>,
}

pub(super) enum LoginAttemptOutcome {
    Success(AuthenticatedSession),
    InvalidCredentials { remaining_lock_secs: i64 },
    InvalidTotp { remaining_lock_secs: i64 },
    Locked { remaining_lock_secs: i64 },
    RequiresTotp,
}

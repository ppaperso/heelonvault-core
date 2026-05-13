pub mod access_control;
pub mod admin_service;
pub mod audit_log_service;
pub mod audit_report_service;
pub mod audit_service;
pub mod auth_policy_service;
pub mod auth_service;
pub mod backup_application_service;
pub mod backup_service;
pub mod crypto_service;
pub mod import_service;
pub mod license_service;
pub mod login_history_service;
pub mod password_service;
pub mod secret_service;
pub mod team_service;
pub mod totp_service;
pub mod user_service;
pub mod vault_service;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("service error placeholder")]
    Placeholder,
}

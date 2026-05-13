use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

use sqlx::SqlitePool;

use crate::errors::AppError;

/// Audit log severity levels for compliance filtering and alerting.
/// Used to prioritize audit events for security monitoring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuditSeverity {
    Info,     // Standard operations (info-level events)
    Warn,     // Security-relevant operations (potential threats)
    Critical, // Security incidents and critical changes
}

impl AuditSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Critical => "CRITICAL",
        }
    }
}

/// Action types for audit logging conform to ISO 15189, RGPD, and SOX.
///
/// Using dot-namespaced action names allows efficient filtering by subsystem:
/// - `bootstrap.*` — first-admin initialization
/// - `auth.*` — authentication flows (login/logout)
/// - `rbac.*` — role-based access control changes (CRITICAL)
/// - `secret.*` — secret CRUD operations
/// - `license.*` — license validation
#[derive(Clone, Debug)]
pub enum AuditAction {
    BootstrapInit,        // First admin account creation
    BootstrapInitSuccess, // Bootstrap succeeded
    BootstrapInitFailure, // Bootstrap failed
    AuthLogin,            // Login attempt
    AuthLoginSuccess,     // Login succeeded
    AuthLoginFailure,     // Login failed (WARN severity)
    AuthLogout,           // Logout
    RbacPermissionChange, // RBAC permission modified (CRITICAL severity)
    RbacRoleAssignment,   // Role assigned to user (CRITICAL severity)
    RbacRoleRevocation,   // Role revoked from user (CRITICAL severity)
    SecretView,           // Secret viewed (INFO severity)
    SecretCreate,         // Secret created
    SecretUpdate,         // Secret updated
    SecretDelete,         // Secret deleted (WARN severity)
    LicenseCheck,         // License validation at startup
    LicenseCheckSuccess,  // License valid
    LicenseCheckFailure,  // License invalid
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BootstrapInit => "bootstrap.init",
            Self::BootstrapInitSuccess => "bootstrap.init.success",
            Self::BootstrapInitFailure => "bootstrap.init.failure",
            Self::AuthLogin => "auth.login",
            Self::AuthLoginSuccess => "auth.login.success",
            Self::AuthLoginFailure => "auth.login.failure",
            Self::AuthLogout => "auth.logout",
            Self::RbacPermissionChange => "rbac.permission.change",
            Self::RbacRoleAssignment => "rbac.role.assignment",
            Self::RbacRoleRevocation => "rbac.role.revocation",
            Self::SecretView => "secret.view",
            Self::SecretCreate => "secret.create",
            Self::SecretUpdate => "secret.update",
            Self::SecretDelete => "secret.delete",
            Self::LicenseCheck => "license.check",
            Self::LicenseCheckSuccess => "license.check.success",
            Self::LicenseCheckFailure => "license.check.failure",
        }
    }

    /// Returns the recommended severity level for this action.
    /// Used for audit log filtering and security alerting.
    pub fn severity(&self) -> AuditSeverity {
        match self {
            // CRITICAL: Security incidents and critical permission changes
            Self::RbacPermissionChange
            | Self::RbacRoleAssignment
            | Self::RbacRoleRevocation
            | Self::AuthLoginFailure
            | Self::BootstrapInitFailure => AuditSeverity::Critical,

            // WARN: Security-relevant operations (deletions, failures)
            Self::SecretDelete | Self::AuthLogout => AuditSeverity::Warn,

            // INFO: Standard operations
            _ => AuditSeverity::Info,
        }
    }
}

/// High-performance, non-blocking audit service for compliance logging.
///
/// All operations are spawned to background tasks to prevent UI blocking.
/// Designed for ISO 15189, RGPD, SOX, and medical-lab compliance.
#[derive(Clone)]
pub struct AuditService {
    db_pool: Arc<SqlitePool>,
}

impl AuditService {
    pub fn new(db_pool: SqlitePool) -> Self {
        Self {
            db_pool: Arc::new(db_pool),
        }
    }

    /// Log an audit event asynchronously (non-blocking).
    ///
    /// This spawns a background task and completes immediately.
    /// The actual database write happens out-of-band.
    pub fn log_async(
        &self,
        actor_user_id: Option<Uuid>,
        action: AuditAction,
        target_type: Option<&str>,
        target_id: Option<&str>,
        detail: Option<&str>,
    ) {
        let pool = Arc::clone(&self.db_pool);
        let actor_id_str = actor_user_id.map(|id| id.to_string());
        let target_type_str = target_type.map(|s| s.to_string());
        let target_id_str = target_id.map(|s| s.to_string());
        let detail_str = detail.map(|s| s.to_string());
        let action_str = action.as_str().to_string();

        // Spawn background task — non-blocking.
        tokio::spawn(async move {
            if let Err(e) = sqlx::query(
                r#"
                INSERT INTO audit_log (actor_user_id, action, target_type, target_id, detail)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(actor_id_str)
            .bind(action_str)
            .bind(target_type_str)
            .bind(target_id_str)
            .bind(detail_str)
            .execute(pool.as_ref())
            .await
            {
                warn!("audit log write failed: {:?}", e);
            }
        });
    }

    /// Log an audit event synchronously (blocking, returns Result).
    ///
    /// Used for critical operations that must complete before proceeding
    /// (e.g., bootstrap completion verification).
    pub async fn log_sync(
        &self,
        actor_user_id: Option<Uuid>,
        action: AuditAction,
        target_type: Option<&str>,
        target_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        let actor_id_str = actor_user_id.map(|id| id.to_string());
        let target_type_str = target_type.map(|s| s.to_string());
        let target_id_str = target_id.map(|s| s.to_string());
        let detail_str = detail.map(|s| s.to_string());
        let action_str = action.as_str();

        sqlx::query(
            r#"
            INSERT INTO audit_log (actor_user_id, action, target_type, target_id, detail)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(actor_id_str)
        .bind(action_str)
        .bind(target_type_str)
        .bind(target_id_str)
        .bind(detail_str)
        .execute(self.db_pool.as_ref())
        .await?;

        Ok(())
    }

    /// Convenience: log bootstrap success.
    pub fn log_bootstrap_success(&self, username: &str) {
        self.log_async(
            None, // No user ID yet; bootstrap happens before admin exists
            AuditAction::BootstrapInitSuccess,
            Some("user"),
            Some(username),
            Some("First admin account initialized"),
        );
    }

    /// Convenience: log bootstrap failure.
    pub fn log_bootstrap_failure(&self, reason: &str) {
        self.log_async(
            None,
            AuditAction::BootstrapInitFailure,
            Some("bootstrap"),
            None,
            Some(reason),
        );
    }

    /// Convenience: log login success.
    pub fn log_login_success(&self, user_id: Uuid, username: &str) {
        self.log_async(
            Some(user_id),
            AuditAction::AuthLoginSuccess,
            Some("user"),
            Some(username),
            Some(&format!("Login successful for user {}", username)),
        );
    }

    /// Convenience: log login failure.
    pub fn log_login_failure(&self, username: &str, reason: &str) {
        self.log_async(
            None, // Actor not yet identified on failed login
            AuditAction::AuthLoginFailure,
            Some("user"),
            Some(username),
            Some(&format!("Login failed: {}", reason)),
        );
    }

    /// Convenience: log secret view.
    pub fn log_secret_view(&self, user_id: Uuid, secret_id: Uuid, secret_title: &str) {
        self.log_async(
            Some(user_id),
            AuditAction::SecretView,
            Some("secret"),
            Some(&secret_id.to_string()),
            Some(&format!("Viewed secret: {}", secret_title)),
        );
    }

    /// Convenience: log license check.
    pub fn log_license_check_success(&self) {
        self.log_async(
            None,
            AuditAction::LicenseCheckSuccess,
            Some("license"),
            None,
            Some("License validation passed at startup"),
        );
    }

    /// Convenience: log license check failure.
    pub fn log_license_check_failure(&self, reason: &str) {
        self.log_async(
            None,
            AuditAction::LicenseCheckFailure,
            Some("license"),
            None,
            Some(reason),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_action_strings() {
        assert_eq!(
            AuditAction::BootstrapInitSuccess.as_str(),
            "bootstrap.init.success"
        );
        assert_eq!(AuditAction::AuthLoginSuccess.as_str(), "auth.login.success");
        assert_eq!(AuditAction::SecretView.as_str(), "secret.view");
    }
}

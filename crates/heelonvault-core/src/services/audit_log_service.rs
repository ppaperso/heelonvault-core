use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AuditAction, AuditLogEntry};

// ── trait ─────────────────────────────────────────────────────────────────────

#[trait_variant::make(AuditLogService: Send)]
pub trait LocalAuditLogService {
    async fn record_event(
        &self,
        actor_user_id: Option<Uuid>,
        action: AuditAction,
        target_type: Option<&str>,
        target_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), AppError>;

    /// Returns the `limit` most recent audit entries.  Only admins may call this.
    async fn list_recent(
        &self,
        requester_id: Uuid,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError>;

    /// Returns audit entries performed by a specific actor.  Only admins or the
    /// actor themselves may call this.
    async fn list_for_user(
        &self,
        requester_id: Uuid,
        actor_id: Uuid,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError>;

    /// Returns audit entries that touch a specific target object (e.g. a vault).
    /// Only admins may call this.
    async fn list_for_target(
        &self,
        requester_id: Uuid,
        target_type: &str,
        target_id: &str,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError>;
}

// ── community stub ──────────────────────────────────────────────────────────

/// No-op implementation used in Community builds.
/// All write operations succeed silently; all read operations return empty lists.
pub struct NoOpAuditLogService;

impl AuditLogService for NoOpAuditLogService {
    async fn record_event(
        &self,
        _actor_user_id: Option<Uuid>,
        _action: AuditAction,
        _target_type: Option<&str>,
        _target_id: Option<&str>,
        _detail: Option<&str>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn list_recent(
        &self,
        _requester_id: Uuid,
        _limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        Ok(vec![])
    }

    async fn list_for_user(
        &self,
        _requester_id: Uuid,
        _actor_id: Uuid,
        _limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        Ok(vec![])
    }

    async fn list_for_target(
        &self,
        _requester_id: Uuid,
        _target_type: &str,
        _target_id: &str,
        _limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        Ok(vec![])
    }
}

use uuid::Uuid;

use heelonvault_core::errors::{AccessDeniedReason, AppError};
use heelonvault_core::models::{AuditAction, AuditLogEntry, UserRole};
use heelonvault_core::repositories::audit_log_repository::AuditLogRepository;
use heelonvault_core::repositories::user_repository::UserRepository;
use heelonvault_core::services::audit_log_service::AuditLogService;

pub struct AuditLogServiceImpl<TUserRepo, TAudit>
where
    TUserRepo: UserRepository + Send + Sync,
    TAudit: AuditLogRepository + Send + Sync,
{
    user_repo: TUserRepo,
    audit_repo: TAudit,
}

impl<TUserRepo, TAudit> AuditLogServiceImpl<TUserRepo, TAudit>
where
    TUserRepo: UserRepository + Send + Sync,
    TAudit: AuditLogRepository + Send + Sync,
{
    pub fn new(user_repo: TUserRepo, audit_repo: TAudit) -> Self {
        Self {
            user_repo,
            audit_repo,
        }
    }

    async fn require_admin(&self, requester_id: Uuid) -> Result<(), AppError> {
        let user = self
            .user_repo
            .get_by_id(requester_id)
            .await?
            .ok_or_else(|| AppError::NotFound("requester not found".to_string()))?;
        if !matches!(user.role, UserRole::Admin) {
            return Err(AppError::Authorization(AccessDeniedReason::AdminRequired));
        }
        Ok(())
    }
}

impl<TUserRepo, TAudit> AuditLogService for AuditLogServiceImpl<TUserRepo, TAudit>
where
    TUserRepo: UserRepository + Send + Sync,
    TAudit: AuditLogRepository + Send + Sync,
{
    async fn record_event(
        &self,
        actor_user_id: Option<Uuid>,
        action: AuditAction,
        target_type: Option<&str>,
        target_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        self.audit_repo
            .append(actor_user_id, &action, target_type, target_id, detail)
            .await
    }

    async fn list_recent(
        &self,
        requester_id: Uuid,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        self.require_admin(requester_id).await?;
        self.audit_repo.list_recent(limit).await
    }

    async fn list_for_user(
        &self,
        requester_id: Uuid,
        actor_id: Uuid,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        let requester = self
            .user_repo
            .get_by_id(requester_id)
            .await?
            .ok_or_else(|| AppError::NotFound("requester not found".to_string()))?;

        if !matches!(requester.role, UserRole::Admin) && requester_id != actor_id {
            return Err(AppError::Authorization(
                AccessDeniedReason::AuditCrossUserDenied,
            ));
        }

        self.audit_repo.list_for_actor(actor_id, limit).await
    }

    async fn list_for_target(
        &self,
        requester_id: Uuid,
        target_type: &str,
        target_id: &str,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        self.require_admin(requester_id).await?;
        self.audit_repo
            .list_for_target(target_type, target_id, limit)
            .await
    }
}

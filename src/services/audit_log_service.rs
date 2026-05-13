use uuid::Uuid;

use crate::errors::{AccessDeniedReason, AppError};
use crate::models::{AuditAction, AuditLogEntry, UserRole};
use crate::repositories::audit_log_repository::AuditLogRepository;
use crate::repositories::user_repository::UserRepository;

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

// ── implementation ───────────────────────────────────────────────────────────

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
        // Admins can query anyone; regular users can only query themselves.
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

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use uuid::Uuid;

    use crate::errors::AppError;
    use crate::models::{AuditAction, AuditLogEntry, User, UserRole};
    use crate::repositories::audit_log_repository::AuditLogRepository;
    use crate::repositories::user_repository::UserRepository;

    use super::{AuditLogService, AuditLogServiceImpl};

    // ── stubs ─────────────────────────────────────────────────────────────────

    #[derive(Default, Clone)]
    struct StubUserRepo {
        users: Arc<Mutex<HashMap<Uuid, User>>>,
    }

    impl StubUserRepo {
        fn with_admin() -> (Self, Uuid) {
            let id = Uuid::new_v4();
            let repo = Self::default();
            repo.users.lock().unwrap().insert(
                id,
                User {
                    id,
                    username: "admin".to_string(),
                    role: UserRole::Admin,
                    email: None,
                    display_name: None,
                    preferred_language: "fr".to_string(),
                    show_passwords_in_edit: false,
                    updated_at: None,
                },
            );
            (repo, id)
        }

        fn insert_user(&self, role: UserRole) -> Uuid {
            let id = Uuid::new_v4();
            self.users.lock().unwrap().insert(
                id,
                User {
                    id,
                    username: id.to_string(),
                    role,
                    email: None,
                    display_name: None,
                    preferred_language: "fr".to_string(),
                    show_passwords_in_edit: false,
                    updated_at: None,
                },
            );
            id
        }
    }

    impl UserRepository for StubUserRepo {
        async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
            Ok(self.users.lock().unwrap().get(&id).cloned())
        }
        async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .values()
                .find(|u| u.username == username)
                .cloned())
        }
        async fn resolve_username_for_login_identifier(
            &self,
            id: &str,
        ) -> Result<Option<String>, AppError> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .values()
                .find(|u| u.username == id)
                .map(|u| u.username.clone()))
        }
        async fn list_all(&self) -> Result<Vec<User>, AppError> {
            Ok(vec![])
        }
        async fn create_user_db(&self, _: Uuid, _: &str, _: &UserRole) -> Result<(), AppError> {
            Ok(())
        }
        async fn delete_user(&self, _: Uuid) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_user_role(&self, _: Uuid, _: &UserRole) -> Result<(), AppError> {
            Ok(())
        }
        async fn list_all_password_envelopes(&self) -> Result<Vec<(String, Vec<u8>)>, AppError> {
            Ok(vec![])
        }
        async fn get_password_envelope_by_user_id(
            &self,
            _: Uuid,
        ) -> Result<Option<secrecy::SecretBox<Vec<u8>>>, AppError> {
            Ok(None)
        }
        async fn update_user_profile(
            &self,
            _: Uuid,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<bool>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_password_envelope(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_totp_secret_envelope(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_show_passwords_in_edit(&self, _: Uuid, _: bool) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[derive(Default, Clone)]
    struct StubAuditRepo {
        entries: Arc<Mutex<Vec<AuditLogEntry>>>,
    }

    impl AuditLogRepository for StubAuditRepo {
        async fn append(
            &self,
            actor: Option<Uuid>,
            action: &AuditAction,
            target_type: Option<&str>,
            target_id: Option<&str>,
            detail: Option<&str>,
        ) -> Result<(), AppError> {
            let mut guard = self.entries.lock().unwrap();
            let id = guard.len() as i64 + 1;
            guard.push(AuditLogEntry {
                id,
                actor_user_id: actor,
                action: action.to_db_str().to_string(),
                target_type: target_type.map(|s| s.to_string()),
                target_id: target_id.map(|s| s.to_string()),
                detail: detail.map(|s| s.to_string()),
                performed_at: "2026-01-01T00:00:00".to_string(),
            });
            Ok(())
        }
        async fn list_recent(&self, limit: u32) -> Result<Vec<AuditLogEntry>, AppError> {
            let guard = self.entries.lock().unwrap();
            let n = limit as usize;
            Ok(guard.iter().rev().take(n).cloned().collect())
        }
        async fn list_for_actor(
            &self,
            actor_id: Uuid,
            limit: u32,
        ) -> Result<Vec<AuditLogEntry>, AppError> {
            let guard = self.entries.lock().unwrap();
            Ok(guard
                .iter()
                .filter(|e| e.actor_user_id == Some(actor_id))
                .take(limit as usize)
                .cloned()
                .collect())
        }
        async fn list_for_target(
            &self,
            tt: &str,
            tid: &str,
            limit: u32,
        ) -> Result<Vec<AuditLogEntry>, AppError> {
            let guard = self.entries.lock().unwrap();
            Ok(guard
                .iter()
                .filter(|e| {
                    e.target_type.as_deref() == Some(tt) && e.target_id.as_deref() == Some(tid)
                })
                .take(limit as usize)
                .cloned()
                .collect())
        }
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn non_admin_cannot_list_recent() {
        let (user_repo, _) = StubUserRepo::with_admin();
        let regular_id = user_repo.insert_user(UserRole::User);
        let audit_repo = StubAuditRepo::default();
        let svc = AuditLogServiceImpl::new(user_repo, audit_repo);
        let result = svc.list_recent(regular_id, 10).await;
        assert!(matches!(result, Err(AppError::Authorization(_))));
    }

    #[tokio::test]
    async fn admin_can_list_recent() {
        let (user_repo, admin_id) = StubUserRepo::with_admin();
        let audit_repo = StubAuditRepo::default();
        audit_repo
            .append(
                Some(admin_id),
                &AuditAction::TeamCreated,
                Some("team"),
                Some("t1"),
                None,
            )
            .await
            .unwrap();
        let svc = AuditLogServiceImpl::new(user_repo, audit_repo);
        let entries = svc.list_recent(admin_id, 5).await.expect("list_recent");
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn user_can_view_own_audit_log() {
        let (user_repo, _) = StubUserRepo::with_admin();
        let user_id = user_repo.insert_user(UserRole::User);
        let audit_repo = StubAuditRepo::default();
        audit_repo
            .append(
                Some(user_id),
                &AuditAction::UserCreated,
                Some("user"),
                Some(&user_id.to_string()),
                None,
            )
            .await
            .unwrap();
        let svc = AuditLogServiceImpl::new(user_repo, audit_repo);
        let entries = svc
            .list_for_user(user_id, user_id, 10)
            .await
            .expect("list_for_user");
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn user_cannot_view_other_user_audit_log() {
        let (user_repo, _) = StubUserRepo::with_admin();
        let user_a = user_repo.insert_user(UserRole::User);
        let user_b = user_repo.insert_user(UserRole::User);
        let audit_repo = StubAuditRepo::default();
        let svc = AuditLogServiceImpl::new(user_repo, audit_repo);
        let result = svc.list_for_user(user_a, user_b, 10).await;
        assert!(matches!(result, Err(AppError::Authorization(_))));
    }
}

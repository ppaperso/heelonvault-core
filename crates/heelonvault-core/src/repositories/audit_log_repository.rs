use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AuditAction, AuditLogEntry};

#[trait_variant::make(AuditLogRepository: Send)]
pub trait LocalAuditLogRepository {
    /// Append a single audit record. Non-blocking for the caller: failures are
    /// logged but must not abort the originating operation.
    async fn append(
        &self,
        actor_user_id: Option<Uuid>,
        action: &AuditAction,
        target_type: Option<&str>,
        target_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), AppError>;

    async fn list_recent(&self, limit: u32) -> Result<Vec<AuditLogEntry>, AppError>;

    async fn list_for_actor(
        &self,
        actor_user_id: Uuid,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError>;

    async fn list_for_target(
        &self,
        target_type: &str,
        target_id: &str,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError>;
}

pub struct SqlxAuditLogRepository {
    pool: SqlitePool,
}

impl SqlxAuditLogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<AuditLogEntry, AppError> {
        let id: i64 = row.try_get("id")?;
        let actor_str: Option<String> = row.try_get("actor_user_id")?;
        let action: String = row.try_get("action")?;
        let target_type: Option<String> = row.try_get("target_type")?;
        let target_id: Option<String> = row.try_get("target_id")?;
        let detail: Option<String> = row.try_get("detail")?;
        let performed_at: String = row.try_get("performed_at")?;

        let actor_user_id = actor_str
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|err| AppError::Storage(format!("parse actor_user_id: {err}")))?;

        Ok(AuditLogEntry {
            id,
            actor_user_id,
            action,
            target_type,
            target_id,
            detail,
            performed_at,
        })
    }
}

impl AuditLogRepository for SqlxAuditLogRepository {
    async fn append(
        &self,
        actor_user_id: Option<Uuid>,
        action: &AuditAction,
        target_type: Option<&str>,
        target_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO audit_log (actor_user_id, action, target_type, target_id, detail)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(actor_user_id.map(|u| u.to_string()))
        .bind(action.to_db_str())
        .bind(target_type)
        .bind(target_id)
        .bind(detail)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_recent(&self, limit: u32) -> Result<Vec<AuditLogEntry>, AppError> {
        let rows = sqlx::query(
            "SELECT id, actor_user_id, action, target_type, target_id, detail, performed_at
             FROM audit_log ORDER BY performed_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::row_to_entry).collect()
    }

    async fn list_for_actor(
        &self,
        actor_user_id: Uuid,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        let rows = sqlx::query(
            "SELECT id, actor_user_id, action, target_type, target_id, detail, performed_at
             FROM audit_log WHERE actor_user_id = ?1
             ORDER BY performed_at DESC LIMIT ?2",
        )
        .bind(actor_user_id.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::row_to_entry).collect()
    }

    async fn list_for_target(
        &self,
        target_type: &str,
        target_id: &str,
        limit: u32,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        let rows = sqlx::query(
            "SELECT id, actor_user_id, action, target_type, target_id, detail, performed_at
             FROM audit_log WHERE target_type = ?1 AND target_id = ?2
             ORDER BY performed_at DESC LIMIT ?3",
        )
        .bind(target_type)
        .bind(target_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::row_to_entry).collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{AuditLogRepository, SqlxAuditLogRepository};
    use crate::models::AuditAction;
    use sqlx::sqlite::SqlitePoolOptions;
    use uuid::Uuid;

    async fn setup_repo() -> Result<SqlxAuditLogRepository, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|err| format!("connect in-memory sqlite: {err}"))?;

        sqlx::query(
            "CREATE TABLE audit_log (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                actor_user_id TEXT,
                action        TEXT NOT NULL,
                target_type   TEXT,
                target_id     TEXT,
                detail        TEXT,
                performed_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create audit_log table: {err}"))?;

        Ok(SqlxAuditLogRepository::new(pool))
    }

    #[tokio::test]
    async fn append_and_list_recent() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let actor = Uuid::new_v4();

        let append_result = repo
            .append(
                Some(actor),
                &AuditAction::UserCreated,
                Some("user"),
                Some(&actor.to_string()),
                None,
            )
            .await;
        assert!(append_result.is_ok(), "append should succeed");
        if append_result.is_err() {
            return;
        }

        let entries_result = repo.list_recent(10).await;
        assert!(entries_result.is_ok(), "list_recent should succeed");
        let entries = match entries_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "user.created");
        assert_eq!(entries[0].actor_user_id, Some(actor));
    }

    #[tokio::test]
    async fn list_for_actor_filters_correctly() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let actor_a = Uuid::new_v4();
        let actor_b = Uuid::new_v4();

        let append_a = repo
            .append(
                Some(actor_a),
                &AuditAction::TeamCreated,
                Some("team"),
                Some("t1"),
                None,
            )
            .await;
        assert!(append_a.is_ok(), "append a should succeed");
        if append_a.is_err() {
            return;
        }

        let append_b = repo
            .append(
                Some(actor_b),
                &AuditAction::VaultKeyRotated,
                Some("vault"),
                Some("v1"),
                None,
            )
            .await;
        assert!(append_b.is_ok(), "append b should succeed");
        if append_b.is_err() {
            return;
        }

        let for_a_result = repo.list_for_actor(actor_a, 10).await;
        assert!(for_a_result.is_ok(), "list_for_actor should succeed");
        let for_a = match for_a_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(for_a.len(), 1);
        assert_eq!(for_a[0].action, "team.created");
    }

    #[tokio::test]
    async fn list_for_target_filters_correctly() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let vault_id = Uuid::new_v4().to_string();

        let append_a = repo
            .append(
                None,
                &AuditAction::VaultSharedWithTeam,
                Some("vault"),
                Some(&vault_id),
                None,
            )
            .await;
        assert!(append_a.is_ok(), "append should succeed");
        if append_a.is_err() {
            return;
        }

        let append_b = repo
            .append(
                None,
                &AuditAction::VaultKeyRotated,
                Some("vault"),
                Some(&vault_id),
                None,
            )
            .await;
        assert!(append_b.is_ok(), "append 2 should succeed");
        if append_b.is_err() {
            return;
        }

        let append_c = repo
            .append(
                None,
                &AuditAction::UserDeleted,
                Some("user"),
                Some("u1"),
                None,
            )
            .await;
        assert!(append_c.is_ok(), "append 3 should succeed");
        if append_c.is_err() {
            return;
        }

        let for_vault_result = repo.list_for_target("vault", &vault_id, 10).await;
        assert!(for_vault_result.is_ok(), "list_for_target should succeed");
        let for_vault = match for_vault_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(for_vault.len(), 2);
    }
}

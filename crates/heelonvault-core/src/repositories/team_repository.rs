use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{Team, TeamMember, TeamMemberRole};

#[trait_variant::make(TeamRepository: Send)]
pub trait LocalTeamRepository {
    async fn create_team(
        &self,
        id: Uuid,
        name: &str,
        created_by: Option<Uuid>,
    ) -> Result<Team, AppError>;
    async fn get_by_id(&self, team_id: Uuid) -> Result<Option<Team>, AppError>;
    async fn list_all(&self) -> Result<Vec<Team>, AppError>;
    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Team>, AppError>;
    async fn delete_team(&self, team_id: Uuid) -> Result<(), AppError>;
    async fn add_member(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        role: &TeamMemberRole,
    ) -> Result<(), AppError>;
    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<(), AppError>;
    async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>, AppError>;
    async fn get_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<TeamMemberRole>, AppError>;
    /// Returns the user_ids of all members of a team (for bulk operations).
    async fn list_member_user_ids(&self, team_id: Uuid) -> Result<Vec<Uuid>, AppError>;
    /// Returns every team_id that contains this user, used during user deletion.
    async fn list_team_ids_for_user(&self, user_id: Uuid) -> Result<Vec<Uuid>, AppError>;
}

pub struct SqlxTeamRepository {
    pool: SqlitePool,
}

impl SqlxTeamRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_member_role(raw: &str) -> Result<TeamMemberRole, AppError> {
        TeamMemberRole::from_db_str(raw)
            .map_err(|err| AppError::Storage(format!("invalid team member role in storage: {err}")))
    }

    fn row_to_team(row: &sqlx::sqlite::SqliteRow) -> Result<Team, AppError> {
        let id_str: String = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let created_by_str: Option<String> = row.try_get("created_by")?;
        let created_at: String = row.try_get("created_at")?;

        let id = Uuid::parse_str(&id_str)
            .map_err(|err| AppError::Storage(format!("parse team id: {err}")))?;
        let created_by = created_by_str
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|err| AppError::Storage(format!("parse team created_by: {err}")))?;

        Ok(Team {
            id,
            name,
            created_by,
            created_at,
        })
    }
}

impl TeamRepository for SqlxTeamRepository {
    async fn create_team(
        &self,
        id: Uuid,
        name: &str,
        created_by: Option<Uuid>,
    ) -> Result<Team, AppError> {
        if name.trim().is_empty() {
            return Err(AppError::Validation(
                "team name must not be empty".to_string(),
            ));
        }
        sqlx::query("INSERT INTO teams (id, name, created_by) VALUES (?1, ?2, ?3)")
            .bind(id.to_string())
            .bind(name)
            .bind(created_by.map(|u| u.to_string()))
            .execute(&self.pool)
            .await?;

        let row = sqlx::query("SELECT id, name, created_by, created_at FROM teams WHERE id = ?1")
            .bind(id.to_string())
            .fetch_one(&self.pool)
            .await?;

        Self::row_to_team(&row)
    }

    async fn get_by_id(&self, team_id: Uuid) -> Result<Option<Team>, AppError> {
        let row_opt =
            sqlx::query("SELECT id, name, created_by, created_at FROM teams WHERE id = ?1")
                .bind(team_id.to_string())
                .fetch_optional(&self.pool)
                .await?;

        match row_opt {
            Some(row) => Ok(Some(Self::row_to_team(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_all(&self) -> Result<Vec<Team>, AppError> {
        let rows = sqlx::query("SELECT id, name, created_by, created_at FROM teams ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(Self::row_to_team).collect()
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Team>, AppError> {
        let rows = sqlx::query(
            "SELECT t.id, t.name, t.created_by, t.created_at \
             FROM teams t \
             INNER JOIN team_members tm ON tm.team_id = t.id \
             WHERE tm.user_id = ?1 \
             ORDER BY t.name",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::row_to_team).collect()
    }

    async fn delete_team(&self, team_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM teams WHERE id = ?1")
            .bind(team_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "team not found for deletion".to_string(),
            ));
        }
        Ok(())
    }

    async fn add_member(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        role: &TeamMemberRole,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO team_members (team_id, user_id, role) VALUES (?1, ?2, ?3) \
             ON CONFLICT(team_id, user_id) DO UPDATE SET role = excluded.role",
        )
        .bind(team_id.to_string())
        .bind(user_id.to_string())
        .bind(role.to_db_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM team_members WHERE team_id = ?1 AND user_id = ?2")
            .bind(team_id.to_string())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "team membership not found for removal".to_string(),
            ));
        }
        Ok(())
    }

    async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>, AppError> {
        let rows = sqlx::query(
            "SELECT team_id, user_id, role, joined_at \
             FROM team_members WHERE team_id = ?1 ORDER BY joined_at",
        )
        .bind(team_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut members = Vec::with_capacity(rows.len());
        for row in &rows {
            let team_id_str: String = row.try_get("team_id")?;
            let user_id_str: String = row.try_get("user_id")?;
            let role_raw: String = row.try_get("role")?;
            let joined_at: String = row.try_get("joined_at")?;

            members.push(TeamMember {
                team_id: Uuid::parse_str(&team_id_str)
                    .map_err(|err| AppError::Storage(format!("parse team_id: {err}")))?,
                user_id: Uuid::parse_str(&user_id_str)
                    .map_err(|err| AppError::Storage(format!("parse user_id: {err}")))?,
                role: Self::parse_member_role(&role_raw)?,
                joined_at,
            });
        }
        Ok(members)
    }

    async fn get_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<TeamMemberRole>, AppError> {
        let row_opt =
            sqlx::query("SELECT role FROM team_members WHERE team_id = ?1 AND user_id = ?2")
                .bind(team_id.to_string())
                .bind(user_id.to_string())
                .fetch_optional(&self.pool)
                .await?;

        match row_opt {
            Some(row) => {
                let role_raw: String = row.try_get("role")?;
                Ok(Some(Self::parse_member_role(&role_raw)?))
            }
            None => Ok(None),
        }
    }

    async fn list_member_user_ids(&self, team_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query("SELECT user_id FROM team_members WHERE team_id = ?1")
            .bind(team_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            let user_id_str: String = row.try_get("user_id")?;
            ids.push(
                Uuid::parse_str(&user_id_str)
                    .map_err(|err| AppError::Storage(format!("parse user_id for ids: {err}")))?,
            );
        }
        Ok(ids)
    }

    async fn list_team_ids_for_user(&self, user_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query("SELECT team_id FROM team_members WHERE user_id = ?1")
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            let team_id_str: String = row.try_get("team_id")?;
            ids.push(
                Uuid::parse_str(&team_id_str)
                    .map_err(|err| AppError::Storage(format!("parse team_id for user: {err}")))?,
            );
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{SqlxTeamRepository, TeamRepository};
    use crate::models::TeamMemberRole;
    use sqlx::sqlite::SqlitePoolOptions;
    use uuid::Uuid;

    async fn setup_repo() -> Result<SqlxTeamRepository, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|err| format!("connect in-memory sqlite: {err}"))?;

        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .map_err(|err| format!("enable foreign keys pragma: {err}"))?;

        sqlx::query(
            "CREATE TABLE users (
                id TEXT PRIMARY KEY NOT NULL,
                username TEXT NOT NULL UNIQUE,
                role TEXT NOT NULL DEFAULT 'user'
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create users table: {err}"))?;

        sqlx::query(
            "CREATE TABLE teams (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                created_by TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create teams table: {err}"))?;

        sqlx::query(
            "CREATE TABLE team_members (
                team_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'member',
                joined_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (team_id, user_id),
                FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                CHECK (role IN ('member', 'leader'))
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create team_members table: {err}"))?;

        Ok(SqlxTeamRepository::new(pool))
    }

    async fn seed_user(repo: &SqlxTeamRepository, user_id: Uuid) -> Result<(), String> {
        sqlx::query("INSERT INTO users (id, username) VALUES (?1, ?2)")
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .execute(&repo.pool)
            .await
            .map_err(|err| format!("seed user: {err}"))?;
        Ok(())
    }

    #[tokio::test]
    async fn create_and_get_team() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let team_id = Uuid::new_v4();
        let team_result = repo.create_team(team_id, "DevOps", None).await;
        assert!(team_result.is_ok(), "create_team should succeed");
        let team = match team_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(team.name, "DevOps");
        assert_eq!(team.id, team_id);

        let found_result = repo.get_by_id(team_id).await;
        assert!(found_result.is_ok(), "get_by_id should succeed");
        let found = match found_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(found.is_some());
        let found_team = match found {
            Some(value) => value,
            None => return,
        };
        assert_eq!(found_team.name, "DevOps");
    }

    #[tokio::test]
    async fn add_and_list_members() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let team_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let seed_result = seed_user(&repo, user_id).await;
        assert!(seed_result.is_ok(), "seed user should succeed");
        if seed_result.is_err() {
            return;
        }

        let create_result = repo.create_team(team_id, "Alpha", None).await;
        assert!(create_result.is_ok(), "create_team should succeed");
        if create_result.is_err() {
            return;
        }

        let add_result = repo
            .add_member(team_id, user_id, &TeamMemberRole::Leader)
            .await;
        assert!(add_result.is_ok(), "add_member should succeed");
        if add_result.is_err() {
            return;
        }

        let members_result = repo.list_members(team_id).await;
        assert!(members_result.is_ok(), "list_members should succeed");
        let members = match members_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, user_id);
        assert_eq!(members[0].role, TeamMemberRole::Leader);
    }

    #[tokio::test]
    async fn remove_member_decrement() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let team_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let seed_result = seed_user(&repo, user_id).await;
        assert!(seed_result.is_ok(), "seed user should succeed");
        if seed_result.is_err() {
            return;
        }

        let create_result = repo.create_team(team_id, "Beta", None).await;
        assert!(create_result.is_ok(), "create_team should succeed");
        if create_result.is_err() {
            return;
        }

        let add_result = repo
            .add_member(team_id, user_id, &TeamMemberRole::Member)
            .await;
        assert!(add_result.is_ok(), "add_member should succeed");
        if add_result.is_err() {
            return;
        }

        let remove_result = repo.remove_member(team_id, user_id).await;
        assert!(remove_result.is_ok(), "remove_member should succeed");
        if remove_result.is_err() {
            return;
        }

        let members_result = repo.list_members(team_id).await;
        assert!(members_result.is_ok(), "list_members should succeed");
        let members = match members_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(members.is_empty());
    }

    #[tokio::test]
    async fn delete_team_cascades_members() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let team_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let seed_result = seed_user(&repo, user_id).await;
        assert!(seed_result.is_ok(), "seed user should succeed");
        if seed_result.is_err() {
            return;
        }

        let create_result = repo.create_team(team_id, "Gamma", None).await;
        assert!(create_result.is_ok(), "create_team should succeed");
        if create_result.is_err() {
            return;
        }

        let add_result = repo
            .add_member(team_id, user_id, &TeamMemberRole::Member)
            .await;
        assert!(add_result.is_ok(), "add_member should succeed");
        if add_result.is_err() {
            return;
        }

        let delete_result = repo.delete_team(team_id).await;
        assert!(delete_result.is_ok(), "delete_team should succeed");
        if delete_result.is_err() {
            return;
        }

        let ids_result = repo.list_member_user_ids(team_id).await;
        assert!(ids_result.is_ok(), "list_member_user_ids should succeed");
        let ids = match ids_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(ids.is_empty(), "cascade should remove members");
    }
}

use crate::errors::AppError;
use crate::models::{User, UserRole};
use secrecy::ExposeSecret;
use secrecy::SecretBox;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[trait_variant::make(UserRepository: Send)]
pub trait LocalUserRepository {
    async fn get_by_id(&self, user_id: Uuid) -> Result<Option<User>, AppError>;
    async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError>;
    async fn resolve_username_for_login_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<String>, AppError>;
    async fn list_all(&self) -> Result<Vec<User>, AppError>;
    async fn create_user_db(
        &self,
        user_id: Uuid,
        username: &str,
        role: &UserRole,
    ) -> Result<(), AppError>;
    async fn delete_user(&self, user_id: Uuid) -> Result<(), AppError>;
    async fn update_user_role(&self, user_id: Uuid, role: &UserRole) -> Result<(), AppError>;
    /// Returns (username, password_envelope_bytes) for every user that has an envelope.
    /// Used at startup to populate the in-memory AuthService for all users.
    async fn list_all_password_envelopes(&self) -> Result<Vec<(String, Vec<u8>)>, AppError>;
    async fn get_password_envelope_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError>;
    async fn update_user_profile(
        &self,
        user_id: Uuid,
        email: Option<&str>,
        display_name: Option<&str>,
        preferred_language: Option<&str>,
        show_passwords_in_edit: Option<bool>,
    ) -> Result<(), AppError>;
    async fn update_password_envelope(
        &self,
        user_id: Uuid,
        encrypted_password_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn update_totp_secret_envelope(
        &self,
        user_id: Uuid,
        encrypted_totp_secret_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn update_show_passwords_in_edit(
        &self,
        user_id: Uuid,
        show_passwords_in_edit: bool,
    ) -> Result<(), AppError>;
}

pub struct SqlxUserRepository {
    pool: SqlitePool,
}

impl SqlxUserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_role(role: &str) -> Result<UserRole, AppError> {
        match role {
            "user" => Ok(UserRole::User),
            "admin" => Ok(UserRole::Admin),
            _ => Err(AppError::Storage(
                "invalid user role in storage".to_string(),
            )),
        }
    }

    fn format_role(role: &UserRole) -> &'static str {
        match role {
            UserRole::Admin => "admin",
            UserRole::User => "user",
        }
    }
}

impl UserRepository for SqlxUserRepository {
    async fn get_by_id(&self, user_id: Uuid) -> Result<Option<User>, AppError> {
        let row_opt = sqlx::query(
            "SELECT id, username, role, email, display_name, COALESCE(NULLIF(preferred_language, ''), 'fr') AS preferred_language, show_passwords_in_edit, updated_at FROM users WHERE id = ?1",
        )
            .bind(user_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row_opt {
            Some(row) => {
                let id_str: String = row.try_get("id")?;
                let username: String = row.try_get("username")?;
                let role_raw: String = row.try_get("role")?;
                let email: Option<String> = row.try_get("email")?;
                let display_name: Option<String> = row.try_get("display_name")?;
                let preferred_language: String = row.try_get("preferred_language")?;
                let show_passwords_in_edit: i64 = row.try_get("show_passwords_in_edit")?;
                let updated_at: Option<String> = row.try_get("updated_at")?;

                let parsed_id = Uuid::parse_str(&id_str)
                    .map_err(|err| AppError::Storage(format!("parse user id: {err}")))?;
                let role = Self::parse_role(&role_raw)?;

                Ok(Some(User {
                    id: parsed_id,
                    username,
                    role,
                    email,
                    display_name,
                    preferred_language,
                    show_passwords_in_edit: show_passwords_in_edit != 0,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        let row_opt = sqlx::query(
            "SELECT id, username, role, email, display_name, COALESCE(NULLIF(preferred_language, ''), 'fr') AS preferred_language, show_passwords_in_edit, updated_at FROM users WHERE username = ?1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => {
                let id_str: String = row.try_get("id")?;
                let stored_username: String = row.try_get("username")?;
                let role_raw: String = row.try_get("role")?;
                let email: Option<String> = row.try_get("email")?;
                let display_name: Option<String> = row.try_get("display_name")?;
                let preferred_language: String = row.try_get("preferred_language")?;
                let show_passwords_in_edit: i64 = row.try_get("show_passwords_in_edit")?;
                let updated_at: Option<String> = row.try_get("updated_at")?;

                let parsed_id = Uuid::parse_str(&id_str)
                    .map_err(|err| AppError::Storage(format!("parse user id: {err}")))?;
                let role = Self::parse_role(&role_raw)?;

                Ok(Some(User {
                    id: parsed_id,
                    username: stored_username,
                    role,
                    email,
                    display_name,
                    preferred_language,
                    show_passwords_in_edit: show_passwords_in_edit != 0,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn resolve_username_for_login_identifier(
        &self,
        identifier: &str,
    ) -> Result<Option<String>, AppError> {
        let normalized_identifier = identifier.trim();
        if normalized_identifier.is_empty() {
            return Ok(None);
        }

        let row_opt = sqlx::query(
            "SELECT username
             FROM (
                 SELECT username, 1 AS match_rank
                 FROM users
                 WHERE lower(trim(username)) = lower(trim(?1))

                 UNION ALL

                 SELECT username, 2 AS match_rank
                 FROM users
                 WHERE email IS NOT NULL
                   AND trim(email) <> ''
                   AND lower(trim(email)) = lower(trim(?1))

                 UNION ALL

                 SELECT username, 3 AS match_rank
                 FROM users
                 WHERE display_name IS NOT NULL
                   AND trim(display_name) <> ''
                   AND lower(trim(display_name)) = lower(trim(?1))
             )
             ORDER BY match_rank ASC, username ASC
             LIMIT 1",
        )
        .bind(normalized_identifier)
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => {
                let username: String = row.try_get("username")?;
                Ok(Some(username))
            }
            None => Ok(None),
        }
    }

    async fn get_password_envelope_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        let row_opt = sqlx::query("SELECT password_envelope FROM users WHERE id = ?1")
            .bind(user_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row_opt {
            Some(row) => {
                let envelope_bytes: Option<Vec<u8>> = row.try_get("password_envelope")?;
                Ok(envelope_bytes.map(|value| SecretBox::new(Box::new(value))))
            }
            None => Ok(None),
        }
    }

    async fn update_user_profile(
        &self,
        user_id: Uuid,
        email: Option<&str>,
        display_name: Option<&str>,
        preferred_language: Option<&str>,
        show_passwords_in_edit: Option<bool>,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE users
             SET email = ?1,
                 display_name = ?2,
                 preferred_language = COALESCE(?3, preferred_language),
                 show_passwords_in_edit = COALESCE(?4, show_passwords_in_edit),
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?5",
        )
        .bind(email)
        .bind(display_name)
        .bind(preferred_language)
        .bind(show_passwords_in_edit.map(|value| if value { 1_i64 } else { 0_i64 }))
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "user not found for profile update".to_string(),
            ));
        }

        Ok(())
    }

    async fn update_password_envelope(
        &self,
        user_id: Uuid,
        encrypted_password_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE users SET password_envelope = ?1 WHERE id = ?2")
            .bind(encrypted_password_envelope.expose_secret().as_slice())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "user not found for password update".to_string(),
            ));
        }

        Ok(())
    }

    async fn update_totp_secret_envelope(
        &self,
        user_id: Uuid,
        encrypted_totp_secret_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE users SET totp_secret_envelope = ?1 WHERE id = ?2")
            .bind(encrypted_totp_secret_envelope.expose_secret().as_slice())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "user not found for totp update".to_string(),
            ));
        }

        Ok(())
    }

    async fn update_show_passwords_in_edit(
        &self,
        user_id: Uuid,
        show_passwords_in_edit: bool,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE users
             SET show_passwords_in_edit = ?1,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
        )
        .bind(if show_passwords_in_edit { 1_i64 } else { 0_i64 })
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "user not found for show_passwords_in_edit update".to_string(),
            ));
        }

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<User>, AppError> {
        let rows = sqlx::query(
            "SELECT id, username, role, email, display_name, \
             COALESCE(NULLIF(preferred_language, ''), 'fr') AS preferred_language, \
             show_passwords_in_edit, updated_at \
             FROM users ORDER BY username",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            let id_str: String = row.try_get("id")?;
            let username: String = row.try_get("username")?;
            let role_raw: String = row.try_get("role")?;
            let email: Option<String> = row.try_get("email")?;
            let display_name: Option<String> = row.try_get("display_name")?;
            let preferred_language: String = row.try_get("preferred_language")?;
            let show_passwords_in_edit: i64 = row.try_get("show_passwords_in_edit")?;
            let updated_at: Option<String> = row.try_get("updated_at")?;
            let parsed_id = Uuid::parse_str(&id_str)
                .map_err(|err| AppError::Storage(format!("parse user id: {err}")))?;
            let role = Self::parse_role(&role_raw)?;
            users.push(User {
                id: parsed_id,
                username,
                role,
                email,
                display_name,
                preferred_language,
                show_passwords_in_edit: show_passwords_in_edit != 0,
                updated_at,
            });
        }
        Ok(users)
    }

    async fn create_user_db(
        &self,
        user_id: Uuid,
        username: &str,
        role: &UserRole,
    ) -> Result<(), AppError> {
        sqlx::query("INSERT INTO users (id, username, role) VALUES (?1, ?2, ?3)")
            .bind(user_id.to_string())
            .bind(username)
            .bind(Self::format_role(role))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_user(&self, user_id: Uuid) -> Result<(), AppError> {
        let user_id_str = user_id.to_string();

        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE id = ?1")
            .bind(&user_id_str)
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            return Err(AppError::NotFound(
                "user not found for deletion".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await?;

        // login_history has no ON DELETE CASCADE — must be cleaned up manually
        sqlx::query("DELETE FROM login_history WHERE user_id = ?1")
            .bind(&user_id_str)
            .execute(&mut *tx)
            .await?;

        // All other FK references cascade automatically (vaults → secret_items,
        // team_members, vault_key_shares) or are SET NULL (audit_log, teams.created_by,
        // accessible_vaults).
        sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(&user_id_str)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn update_user_role(&self, user_id: Uuid, role: &UserRole) -> Result<(), AppError> {
        let result =
            sqlx::query("UPDATE users SET role = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2")
                .bind(Self::format_role(role))
                .bind(user_id.to_string())
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(
                "user not found for role update".to_string(),
            ));
        }
        Ok(())
    }

    async fn list_all_password_envelopes(&self) -> Result<Vec<(String, Vec<u8>)>, AppError> {
        let rows = sqlx::query(
            "SELECT username, password_envelope FROM users \
             WHERE password_envelope IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let username: String = row.try_get("username")?;
            let envelope: Vec<u8> = row.try_get("password_envelope")?;
            result.push((username, envelope));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{SqlxUserRepository, UserRepository};
    use crate::errors::AppError;
    use crate::models::UserRole;
    use secrecy::SecretBox;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::Row;
    use uuid::Uuid;

    async fn setup_repo() -> Result<SqlxUserRepository, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|err| format!("connect in-memory sqlite: {err}"))?;

        sqlx::query(
            "CREATE TABLE users (
                id TEXT PRIMARY KEY NOT NULL,
                username TEXT NOT NULL UNIQUE,
                role TEXT NOT NULL,
                email TEXT,
                display_name TEXT,
                preferred_language TEXT NOT NULL DEFAULT 'fr',
                show_passwords_in_edit INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT,
                password_envelope BLOB,
                totp_secret_envelope BLOB
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create users table: {err}"))?;

        Ok(SqlxUserRepository::new(pool))
    }

    async fn insert_user(
        repo: &SqlxUserRepository,
        user_id: Uuid,
        username: &str,
        role: &str,
    ) -> Result<(), String> {
        sqlx::query("INSERT INTO users (id, username, role) VALUES (?1, ?2, ?3)")
            .bind(user_id.to_string())
            .bind(username)
            .bind(role)
            .execute(&repo.pool)
            .await
            .map_err(|err| format!("insert user: {err}"))?;
        Ok(())
    }

    #[tokio::test]
    async fn get_user_by_id_maps_model() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let user_id = Uuid::new_v4();
        let insert_result = insert_user(&repo, user_id, "alice", "admin").await;
        assert!(insert_result.is_ok(), "seed user should succeed");
        if insert_result.is_err() {
            return;
        }

        let found_result = repo.get_by_id(user_id).await;
        assert!(found_result.is_ok(), "query by id should succeed");
        let found = match found_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(found.is_some(), "user should be found");
        let user = match found {
            Some(value) => value,
            None => return,
        };

        assert_eq!(user.id, user_id);
        assert_eq!(user.username, "alice");
        assert!(matches!(user.role, UserRole::Admin));
    }

    #[tokio::test]
    async fn get_user_by_username_maps_model() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let user_id = Uuid::new_v4();
        let insert_result = insert_user(&repo, user_id, "bob", "user").await;
        assert!(insert_result.is_ok(), "seed user should succeed");
        if insert_result.is_err() {
            return;
        }

        let found_result = repo.get_by_username("bob").await;
        assert!(found_result.is_ok(), "query by username should succeed");
        let found = match found_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(found.is_some(), "user should be found");
        let user = match found {
            Some(value) => value,
            None => return,
        };

        assert_eq!(user.id, user_id);
        assert_eq!(user.username, "bob");
        assert!(matches!(user.role, UserRole::User));
    }

    #[tokio::test]
    async fn resolve_username_for_login_identifier_matches_username_email_and_display_name() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let user_id = Uuid::new_v4();
        let insert_result = insert_user(&repo, user_id, "alice", "admin").await;
        assert!(insert_result.is_ok(), "seed user should succeed");
        if insert_result.is_err() {
            return;
        }

        let profile_result =
            sqlx::query("UPDATE users SET email = ?1, display_name = ?2 WHERE id = ?3")
                .bind("alice@example.com")
                .bind("Alice Martin")
                .bind(user_id.to_string())
                .execute(&repo.pool)
                .await;
        assert!(profile_result.is_ok(), "profile update should succeed");
        if profile_result.is_err() {
            return;
        }

        let by_username = repo.resolve_username_for_login_identifier("ALICE").await;
        assert!(by_username.is_ok(), "username resolution should succeed");
        assert_eq!(by_username.ok().flatten().as_deref(), Some("alice"));

        let by_email = repo
            .resolve_username_for_login_identifier("  Alice@Example.com ")
            .await;
        assert!(by_email.is_ok(), "email resolution should succeed");
        assert_eq!(by_email.ok().flatten().as_deref(), Some("alice"));

        let by_display = repo
            .resolve_username_for_login_identifier("alice martin")
            .await;
        assert!(by_display.is_ok(), "display name resolution should succeed");
        assert_eq!(by_display.ok().flatten().as_deref(), Some("alice"));

        let missing = repo.resolve_username_for_login_identifier("inconnu").await;
        assert!(missing.is_ok(), "missing lookup should still succeed");
        assert!(
            missing.ok().flatten().is_none(),
            "missing lookup returns none"
        );
    }

    #[tokio::test]
    async fn update_password_envelope_persists_blob() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let user_id = Uuid::new_v4();
        let insert_result = insert_user(&repo, user_id, "carol", "user").await;
        assert!(insert_result.is_ok(), "seed user should succeed");
        if insert_result.is_err() {
            return;
        }

        let envelope = SecretBox::new(Box::new(vec![1_u8, 2_u8, 3_u8, 4_u8]));
        let update_result = repo.update_password_envelope(user_id, envelope).await;
        assert!(update_result.is_ok(), "password update should succeed");
        if update_result.is_err() {
            return;
        }

        let row_result = sqlx::query("SELECT password_envelope FROM users WHERE id = ?1")
            .bind(user_id.to_string())
            .fetch_one(&repo.pool)
            .await;
        assert!(row_result.is_ok(), "readback query should succeed");
        let row = match row_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let stored_result: Result<Vec<u8>, _> = row.try_get("password_envelope");
        assert!(stored_result.is_ok(), "stored blob should be readable");
        let stored = match stored_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(stored, vec![1_u8, 2_u8, 3_u8, 4_u8]);
    }

    #[tokio::test]
    async fn update_totp_secret_envelope_persists_blob() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let user_id = Uuid::new_v4();
        let insert_result = insert_user(&repo, user_id, "dave", "admin").await;
        assert!(insert_result.is_ok(), "seed user should succeed");
        if insert_result.is_err() {
            return;
        }

        let envelope = SecretBox::new(Box::new(vec![9_u8, 8_u8, 7_u8]));
        let update_result = repo.update_totp_secret_envelope(user_id, envelope).await;
        assert!(update_result.is_ok(), "totp update should succeed");
        if update_result.is_err() {
            return;
        }

        let row_result = sqlx::query("SELECT totp_secret_envelope FROM users WHERE id = ?1")
            .bind(user_id.to_string())
            .fetch_one(&repo.pool)
            .await;
        assert!(row_result.is_ok(), "readback query should succeed");
        let row = match row_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let stored_result: Result<Vec<u8>, _> = row.try_get("totp_secret_envelope");
        assert!(stored_result.is_ok(), "stored blob should be readable");
        let stored = match stored_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(stored, vec![9_u8, 8_u8, 7_u8]);
    }

    #[tokio::test]
    async fn updates_missing_user_return_storage_error() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let missing_id = Uuid::new_v4();
        let password_result = repo
            .update_password_envelope(missing_id, SecretBox::new(Box::new(vec![1_u8, 2_u8, 3_u8])))
            .await;
        assert!(password_result.is_err(), "missing user should error");

        let totp_result = repo
            .update_totp_secret_envelope(
                missing_id,
                SecretBox::new(Box::new(vec![4_u8, 5_u8, 6_u8])),
            )
            .await;
        assert!(totp_result.is_err(), "missing user should error");

        if let Err(err) = password_result {
            assert!(matches!(err, AppError::Storage(_)));
        }
        if let Err(err) = totp_result {
            assert!(matches!(err, AppError::Storage(_)));
        }
    }
}

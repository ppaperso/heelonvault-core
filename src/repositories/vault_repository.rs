use crate::errors::AppError;
use crate::models::{AccessibleVault, Vault, VaultAccessKind, VaultShareRole};
use secrecy::ExposeSecret;
use secrecy::SecretBox;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

pub type VaultKeyShareEnvelope = (Uuid, SecretBox<Vec<u8>>, Option<Uuid>);

#[trait_variant::make(VaultRepository: Send)]
pub trait LocalVaultRepository {
    async fn get_by_id(&self, vault_id: Uuid) -> Result<Option<Vault>, AppError>;
    async fn list_by_user_id(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError>;
    async fn list_owned_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError>;
    async fn list_shared_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError>;
    async fn create_vault(&self, vault: &Vault) -> Result<(), AppError>;
    async fn delete_vault(&self, vault_id: Uuid) -> Result<(), AppError>;
    async fn update_vault_key_envelope(
        &self,
        vault_id: Uuid,
        encrypted_vault_key_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    /// List every vault in the system (admin-only operation).
    async fn list_all(&self) -> Result<Vec<Vault>, AppError>;
    /// List vaults owned by OR shared with a user (via accessible_vaults view).
    async fn list_accessible_by_user(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError>;
    async fn get_accessible_vaults(&self, user_id: Uuid) -> Result<Vec<AccessibleVault>, AppError>;
    async fn get_vault_with_permission(
        &self,
        user_id: Uuid,
        vault_id: Uuid,
    ) -> Result<Option<AccessibleVault>, AppError>;
    // ── vault_key_shares ──────────────────────────────────────────────────────
    async fn insert_key_share(
        &self,
        vault_id: Uuid,
        user_id: Uuid,
        key_envelope: SecretBox<Vec<u8>>,
        granted_by: Option<Uuid>,
        granted_via_team: Option<Uuid>,
        role: VaultShareRole,
    ) -> Result<(), AppError>;
    async fn get_key_share(
        &self,
        vault_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError>;
    async fn delete_key_share(&self, vault_id: Uuid, user_id: Uuid) -> Result<(), AppError>;
    async fn delete_all_key_shares(&self, vault_id: Uuid) -> Result<(), AppError>;
    /// Returns all user_ids that have a key share for this vault.
    /// Used during key rotation to know who needs a new envelope.
    async fn list_key_share_user_ids(&self, vault_id: Uuid) -> Result<Vec<Uuid>, AppError>;
    /// Atomic replacement of all key shares for a vault (used during key rotation).
    /// Deletes existing shares then inserts the new ones inside a transaction.
    async fn replace_all_key_shares(
        &self,
        vault_id: Uuid,
        new_shares: &[VaultKeyShareEnvelope],
        rotation_actor_id: Option<Uuid>,
    ) -> Result<(), AppError>;
    async fn delete_key_shares_for_user_via_team(
        &self,
        user_id: Uuid,
        team_id: Uuid,
    ) -> Result<u64, AppError>;
}

pub struct SqlxVaultRepository {
    pool: SqlitePool,
}

impl SqlxVaultRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn rows_to_vaults(rows: Vec<sqlx::sqlite::SqliteRow>) -> Result<Vec<Vault>, sqlx::Error> {
        let mut vaults = Vec::with_capacity(rows.len());
        for row in &rows {
            let id_str: String = row.try_get("id")?;
            let owner_user_id_str: String = row.try_get("owner_user_id")?;
            let name: String = row.try_get("name")?;
            let id = Uuid::parse_str(&id_str)
                .map_err(|_| sqlx::Error::ColumnNotFound("id parse".to_string()))?;
            let owner_user_id = Uuid::parse_str(&owner_user_id_str)
                .map_err(|_| sqlx::Error::ColumnNotFound("owner_user_id parse".to_string()))?;
            vaults.push(Vault {
                id,
                owner_user_id,
                name,
            });
        }
        Ok::<Vec<Vault>, sqlx::Error>(vaults)
    }

    fn row_to_accessible_vault(row: &sqlx::sqlite::SqliteRow) -> Result<AccessibleVault, AppError> {
        let id_str: String = row.try_get("vault_id")?;
        let owner_user_id_str: String = row.try_get("owner_user_id")?;
        let name: String = row.try_get("name")?;
        let role_raw: String = row.try_get("role")?;
        let access_kind_raw: String = row.try_get("access_kind")?;
        let vault_key_version: i64 = row.try_get("vault_key_version")?;

        let id = Uuid::parse_str(&id_str)
            .map_err(|err| AppError::Storage(format!("parse accessible vault id: {err}")))?;
        let owner_user_id = Uuid::parse_str(&owner_user_id_str)
            .map_err(|err| AppError::Storage(format!("parse accessible owner id: {err}")))?;
        let role = VaultShareRole::from_db_str(role_raw.as_str())
            .ok_or_else(|| AppError::Storage("invalid vault share role in storage".to_string()))?;
        let access_kind = VaultAccessKind::from_db_str(access_kind_raw.as_str())
            .ok_or_else(|| AppError::Storage("invalid vault access kind in storage".to_string()))?;

        Ok(AccessibleVault {
            vault: Vault {
                id,
                owner_user_id,
                name,
            },
            role,
            access_kind,
            vault_key_version,
        })
    }
}

impl VaultRepository for SqlxVaultRepository {
    async fn get_by_id(&self, vault_id: Uuid) -> Result<Option<Vault>, AppError> {
        let row_opt = sqlx::query(
            "SELECT id, owner_user_id, name FROM vaults WHERE id = ?1 AND deleted_at IS NULL",
        )
        .bind(vault_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => {
                let id_str: String = row.try_get("id")?;
                let owner_user_id_str: String = row.try_get("owner_user_id")?;
                let name: String = row.try_get("name")?;

                let parsed_id = Uuid::parse_str(&id_str)
                    .map_err(|err| AppError::Storage(format!("parse vault id: {err}")))?;
                let parsed_owner_user_id = Uuid::parse_str(&owner_user_id_str)
                    .map_err(|err| AppError::Storage(format!("parse owner_user_id: {err}")))?;

                Ok(Some(Vault {
                    id: parsed_id,
                    owner_user_id: parsed_owner_user_id,
                    name,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_by_user_id(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
        let rows = sqlx::query(
            "SELECT id, owner_user_id, name FROM vaults WHERE owner_user_id = ?1 AND deleted_at IS NULL ORDER BY LOWER(name)",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut vaults = Vec::with_capacity(rows.len());
        for row in rows {
            let id_str: String = row.try_get("id")?;
            let owner_user_id_str: String = row.try_get("owner_user_id")?;
            let name: String = row.try_get("name")?;

            let parsed_id = Uuid::parse_str(&id_str)
                .map_err(|err| AppError::Storage(format!("parse vault id: {err}")))?;
            let parsed_owner_user_id = Uuid::parse_str(&owner_user_id_str)
                .map_err(|err| AppError::Storage(format!("parse owner_user_id: {err}")))?;

            vaults.push(Vault {
                id: parsed_id,
                owner_user_id: parsed_owner_user_id,
                name,
            });
        }

        Ok(vaults)
    }

    async fn list_owned_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
        VaultRepository::list_by_user_id(self, user_id).await
    }

    async fn list_shared_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
        let rows = sqlx::query(
            "SELECT v.id, v.owner_user_id, v.name
             FROM vaults v
             INNER JOIN vault_key_shares vks ON vks.vault_id = v.id
             WHERE vks.user_id = ?1
               AND v.owner_user_id <> ?1
                             AND v.deleted_at IS NULL
             ORDER BY v.name",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(Self::rows_to_vaults(rows)?)
    }

    async fn create_vault(&self, vault: &Vault) -> Result<(), AppError> {
        sqlx::query("INSERT INTO vaults (id, owner_user_id, name) VALUES (?1, ?2, ?3)")
            .bind(vault.id.to_string())
            .bind(vault.owner_user_id.to_string())
            .bind(&vault.name)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_vault(&self, vault_id: Uuid) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            "UPDATE vaults SET deleted_at = CURRENT_TIMESTAMP WHERE id = ?1 AND deleted_at IS NULL",
        )
        .bind(vault_id.to_string())
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("vault not found".to_string()));
        }

        sqlx::query(
            "UPDATE secret_items SET deleted_at = CURRENT_TIMESTAMP WHERE vault_id = ?1 AND deleted_at IS NULL"
        )
        .bind(vault_id.to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn update_vault_key_envelope(
        &self,
        vault_id: Uuid,
        encrypted_vault_key_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE vaults
             SET vault_key_envelope = ?1,
                 vault_key_version = vault_key_version + 1
             WHERE id = ?2 AND deleted_at IS NULL",
        )
        .bind(encrypted_vault_key_envelope.expose_secret().as_slice())
        .bind(vault_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "vault not found for key update".to_string(),
            ));
        }

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Vault>, AppError> {
        let rows = sqlx::query(
            "SELECT id, owner_user_id, name FROM vaults WHERE deleted_at IS NULL ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(Self::rows_to_vaults(rows)?)
    }

    async fn list_accessible_by_user(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
        let records = VaultRepository::get_accessible_vaults(self, user_id).await?;
        Ok(records.into_iter().map(|record| record.vault).collect())
    }

    async fn get_accessible_vaults(&self, user_id: Uuid) -> Result<Vec<AccessibleVault>, AppError> {
        let rows = sqlx::query(
            "SELECT vault_id, owner_user_id, name, role, access_kind, vault_key_version
             FROM accessible_vaults
             WHERE user_id = ?1
             ORDER BY name",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut records = Vec::with_capacity(rows.len());
        for row in &rows {
            records.push(Self::row_to_accessible_vault(row)?);
        }
        Ok(records)
    }

    async fn get_vault_with_permission(
        &self,
        user_id: Uuid,
        vault_id: Uuid,
    ) -> Result<Option<AccessibleVault>, AppError> {
        let row_opt = sqlx::query(
            "SELECT vault_id, owner_user_id, name, role, access_kind, vault_key_version
             FROM accessible_vaults
             WHERE user_id = ?1 AND vault_id = ?2",
        )
        .bind(user_id.to_string())
        .bind(vault_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => Ok(Some(Self::row_to_accessible_vault(&row)?)),
            None => Ok(None),
        }
    }

    async fn insert_key_share(
        &self,
        vault_id: Uuid,
        user_id: Uuid,
        key_envelope: SecretBox<Vec<u8>>,
        granted_by: Option<Uuid>,
        granted_via_team: Option<Uuid>,
        role: VaultShareRole,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO vault_key_shares (vault_id, user_id, key_envelope, granted_by, granted_via_team, role)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(vault_id, user_id) DO UPDATE SET
                 key_envelope = excluded.key_envelope,
                 granted_by = excluded.granted_by,
                 granted_via_team = excluded.granted_via_team,
                 role = excluded.role,
                 granted_at = CURRENT_TIMESTAMP",
        )
        .bind(vault_id.to_string())
        .bind(user_id.to_string())
        .bind(key_envelope.expose_secret().as_slice())
        .bind(granted_by.map(|u| u.to_string()))
        .bind(granted_via_team.map(|u| u.to_string()))
        .bind(role.to_db_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_key_share(
        &self,
        vault_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        let row_opt = sqlx::query(
            "SELECT key_envelope FROM vault_key_shares WHERE vault_id = ?1 AND user_id = ?2",
        )
        .bind(vault_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => {
                let bytes: Vec<u8> = row.try_get("key_envelope")?;
                Ok(Some(SecretBox::new(Box::new(bytes))))
            }
            None => Ok(None),
        }
    }

    async fn delete_key_share(&self, vault_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM vault_key_shares WHERE vault_id = ?1 AND user_id = ?2")
            .bind(vault_id.to_string())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_all_key_shares(&self, vault_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM vault_key_shares WHERE vault_id = ?1")
            .bind(vault_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_key_share_user_ids(&self, vault_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query("SELECT user_id FROM vault_key_shares WHERE vault_id = ?1")
            .bind(vault_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            let user_id_str: String = row.try_get("user_id")?;
            ids.push(
                Uuid::parse_str(&user_id_str).map_err(|err| {
                    AppError::Storage(format!("parse user_id for key share: {err}"))
                })?,
            );
        }
        Ok(ids)
    }

    async fn replace_all_key_shares(
        &self,
        vault_id: Uuid,
        new_shares: &[VaultKeyShareEnvelope],
        rotation_actor_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM vault_key_shares WHERE vault_id = ?1")
            .bind(vault_id.to_string())
            .execute(&mut *tx)
            .await?;

        for (user_id, key_envelope, granted_via_team) in new_shares {
            sqlx::query(
                "INSERT INTO vault_key_shares \
                 (vault_id, user_id, key_envelope, granted_by, granted_via_team) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(vault_id.to_string())
            .bind(user_id.to_string())
            .bind(key_envelope.expose_secret().as_slice())
            .bind(rotation_actor_id.map(|u| u.to_string()))
            .bind(granted_via_team.map(|u| u.to_string()))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete_key_shares_for_user_via_team(
        &self,
        user_id: Uuid,
        team_id: Uuid,
    ) -> Result<u64, AppError> {
        let result = sqlx::query(
            "DELETE FROM vault_key_shares WHERE user_id = ?1 AND granted_via_team = ?2",
        )
        .bind(user_id.to_string())
        .bind(team_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{SqlxVaultRepository, VaultRepository};
    use crate::errors::AppError;
    use crate::models::Vault;
    use secrecy::SecretBox;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::Row;
    use uuid::Uuid;

    async fn setup_repo() -> Result<SqlxVaultRepository, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|err| format!("connect in-memory sqlite: {err}"))?;

        sqlx::query(
            "CREATE TABLE vaults (
                id TEXT PRIMARY KEY NOT NULL,
                owner_user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                vault_key_envelope BLOB,
                vault_key_version INTEGER NOT NULL DEFAULT 1,
                deleted_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create vaults table: {err}"))?;

        sqlx::query(
            "CREATE TABLE vault_key_shares (
                vault_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                key_envelope BLOB NOT NULL,
                granted_by TEXT,
                granted_via_team TEXT,
                granted_at TEXT,
                role TEXT NOT NULL DEFAULT 'read',
                PRIMARY KEY(vault_id, user_id)
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create vault_key_shares table: {err}"))?;

        sqlx::query(
            "CREATE VIEW accessible_vaults AS
             SELECT owner_user_id AS user_id,
                    id AS vault_id,
                    owner_user_id,
                    name,
                    vault_key_version,
                    'admin' AS role,
                    'owner' AS access_kind
             FROM vaults
             WHERE deleted_at IS NULL
             UNION ALL
             SELECT s.user_id,
                    v.id AS vault_id,
                    v.owner_user_id,
                    v.name,
                    v.vault_key_version,
                    s.role,
                    'direct_share' AS access_kind
             FROM vaults v
             INNER JOIN vault_key_shares s ON s.vault_id = v.id
             WHERE v.deleted_at IS NULL
               AND v.owner_user_id <> s.user_id",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create accessible_vaults view: {err}"))?;

        Ok(SqlxVaultRepository::new(pool))
    }

    async fn seed_vault(
        repo: &SqlxVaultRepository,
        vault_id: Uuid,
        owner_user_id: Uuid,
        name: &str,
    ) -> Result<(), String> {
        sqlx::query("INSERT INTO vaults (id, owner_user_id, name) VALUES (?1, ?2, ?3)")
            .bind(vault_id.to_string())
            .bind(owner_user_id.to_string())
            .bind(name)
            .execute(&repo.pool)
            .await
            .map_err(|err| format!("seed vault: {err}"))?;
        Ok(())
    }

    #[tokio::test]
    async fn create_and_get_vault_by_id() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault = Vault {
            id: Uuid::new_v4(),
            owner_user_id: Uuid::new_v4(),
            name: "Primary".to_string(),
        };

        let create_result = repo.create_vault(&vault).await;
        assert!(create_result.is_ok(), "create_vault should succeed");
        if create_result.is_err() {
            return;
        }

        let found_result = repo.get_by_id(vault.id).await;
        assert!(found_result.is_ok(), "get_by_id should succeed");
        let found_opt = match found_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(found_opt.is_some(), "vault should be found");
        let found = match found_opt {
            Some(value) => value,
            None => return,
        };

        assert_eq!(found.id, vault.id);
        assert_eq!(found.owner_user_id, vault.owner_user_id);
        assert_eq!(found.name, vault.name);
    }

    #[tokio::test]
    async fn list_vaults_by_user_id() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let owner_a = Uuid::new_v4();
        let owner_b = Uuid::new_v4();

        let seed_a1 = seed_vault(&repo, Uuid::new_v4(), owner_a, "Alpha").await;
        let seed_a2 = seed_vault(&repo, Uuid::new_v4(), owner_a, "Beta").await;
        let seed_b1 = seed_vault(&repo, Uuid::new_v4(), owner_b, "Gamma").await;

        assert!(seed_a1.is_ok(), "seed a1 should succeed");
        assert!(seed_a2.is_ok(), "seed a2 should succeed");
        assert!(seed_b1.is_ok(), "seed b1 should succeed");
        if seed_a1.is_err() || seed_a2.is_err() || seed_b1.is_err() {
            return;
        }

        let list_result = repo.list_by_user_id(owner_a).await;
        assert!(list_result.is_ok(), "list_by_user_id should succeed");
        let vaults = match list_result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(vaults.len(), 2);
        assert_eq!(vaults[0].name, "Alpha");
        assert_eq!(vaults[1].name, "Beta");
        assert!(vaults.iter().all(|vault| vault.owner_user_id == owner_a));
    }

    #[tokio::test]
    async fn update_vault_key_envelope_persists_blob() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let seed_result = seed_vault(&repo, vault_id, owner_id, "Secure Vault").await;
        assert!(seed_result.is_ok(), "seed vault should succeed");
        if seed_result.is_err() {
            return;
        }

        let update_result = repo
            .update_vault_key_envelope(vault_id, SecretBox::new(Box::new(vec![3_u8, 1_u8, 4_u8])))
            .await;
        assert!(update_result.is_ok(), "update should succeed");
        if update_result.is_err() {
            return;
        }

        let row_result = sqlx::query("SELECT vault_key_envelope FROM vaults WHERE id = ?1")
            .bind(vault_id.to_string())
            .fetch_one(&repo.pool)
            .await;
        assert!(row_result.is_ok(), "readback should succeed");
        let row = match row_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let blob_result: Result<Vec<u8>, _> = row.try_get("vault_key_envelope");
        assert!(blob_result.is_ok(), "blob should be readable");
        let blob = match blob_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(blob, vec![3_u8, 1_u8, 4_u8]);
    }

    #[tokio::test]
    async fn update_missing_vault_returns_storage_error() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let update_result = repo
            .update_vault_key_envelope(
                Uuid::new_v4(),
                SecretBox::new(Box::new(vec![8_u8, 8_u8, 8_u8])),
            )
            .await;

        assert!(update_result.is_err(), "missing vault update should fail");
        if let Err(err) = update_result {
            assert!(matches!(err, AppError::Storage(_)));
        }
    }
}

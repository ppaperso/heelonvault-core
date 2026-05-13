use crate::errors::AppError;
use crate::models::{BlobStorage, SecretItem, SecretType};
use secrecy::ExposeSecret;
use secrecy::SecretBox;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[trait_variant::make(SecretRepository: Send)]
pub trait LocalSecretRepository {
    async fn get_by_id(&self, secret_id: Uuid) -> Result<Option<SecretItem>, AppError>;
    async fn list_by_vault_id(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError>;
    async fn list_trash_by_vault_id(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError>;
    async fn list_all_trash_by_owner_id(
        &self,
        owner_user_id: Uuid,
    ) -> Result<Vec<SecretItem>, AppError>;
    async fn insert_secret_blob(
        &self,
        item: &SecretItem,
        encrypted_secret_blob: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn update_secret_metadata(
        &self,
        secret_id: Uuid,
        title: Option<String>,
        metadata_json: Option<String>,
        tags: Option<String>,
        expires_at: Option<String>,
    ) -> Result<(), AppError>;
    async fn update_secret_blob(
        &self,
        secret_id: Uuid,
        encrypted_secret_blob: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn increment_usage_count(&self, secret_id: Uuid) -> Result<(), AppError>;
    async fn soft_delete(&self, secret_id: Uuid) -> Result<(), AppError>;
    async fn restore_secret(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError>;
    async fn permanent_delete(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError>;
    async fn empty_trash(&self, vault_id: Uuid) -> Result<usize, AppError>;
}

pub struct SqlxSecretRepository {
    pool: SqlitePool,
}

impl SqlxSecretRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn to_secret_type_db(secret_type: SecretType) -> &'static str {
        match secret_type {
            SecretType::Password => "password",
            SecretType::ApiToken => "api_token",
            SecretType::SshKey => "ssh_key",
            SecretType::SecureDocument => "secure_document",
        }
    }

    fn parse_secret_type_db(raw: &str) -> Result<SecretType, AppError> {
        match raw {
            "password" => Ok(SecretType::Password),
            "api_token" => Ok(SecretType::ApiToken),
            "ssh_key" => Ok(SecretType::SshKey),
            "secure_document" => Ok(SecretType::SecureDocument),
            _ => Err(AppError::Storage(
                "invalid secret_type in storage".to_string(),
            )),
        }
    }

    fn to_blob_storage_db(blob_storage: BlobStorage) -> &'static str {
        match blob_storage {
            BlobStorage::Inline => "inline",
            BlobStorage::File => "file",
        }
    }

    fn parse_blob_storage_db(raw: &str) -> Result<BlobStorage, AppError> {
        match raw {
            "inline" => Ok(BlobStorage::Inline),
            "file" => Ok(BlobStorage::File),
            _ => Err(AppError::Storage(
                "invalid blob_storage in storage".to_string(),
            )),
        }
    }

    fn row_to_secret_item(row: &sqlx::sqlite::SqliteRow) -> Result<SecretItem, AppError> {
        let id_raw: String = row.try_get("id")?;
        let vault_id_raw: String = row.try_get("vault_id")?;
        let secret_type_raw: String = row.try_get("secret_type")?;
        let title: Option<String> = row.try_get("title")?;
        let metadata_json: Option<String> = row.try_get("metadata_json")?;
        let tags: Option<String> = row.try_get("tags")?;
        let expires_at: Option<String> = row.try_get("expires_at")?;
        let created_at: Option<String> = row.try_get("created_at")?;
        let modified_at: Option<String> = row.try_get("modified_at")?;
        let usage_count: u32 = row.try_get("usage_count")?;
        let blob_storage_raw: String = row.try_get("blob_storage")?;

        let id = Uuid::parse_str(&id_raw)
            .map_err(|err| AppError::Storage(format!("parse id: {err}")))?;
        let vault_id = Uuid::parse_str(&vault_id_raw)
            .map_err(|err| AppError::Storage(format!("parse vault_id: {err}")))?;
        let secret_type = Self::parse_secret_type_db(&secret_type_raw)?;
        let blob_storage = Self::parse_blob_storage_db(&blob_storage_raw)?;

        let secret_blob_bytes = match blob_storage {
            BlobStorage::Inline => {
                let raw: Option<Vec<u8>> = row.try_get("secret_blob")?;
                raw.ok_or_else(|| {
                    AppError::Storage("missing inline secret_blob in storage".to_string())
                })?
            }
            BlobStorage::File => {
                let raw: Option<Vec<u8>> = row.try_get("file_blob_ref")?;
                raw.ok_or_else(|| {
                    AppError::Storage("missing file blob reference in storage".to_string())
                })?
            }
        };

        let deleted_at: Option<String> = row.try_get("deleted_at").unwrap_or(None);

        Ok(SecretItem {
            id,
            vault_id,
            secret_type,
            title,
            metadata_json,
            tags,
            expires_at,
            created_at,
            modified_at,
            usage_count,
            blob_storage,
            secret_blob: SecretBox::new(Box::new(secret_blob_bytes)),
            deleted_at,
        })
    }
}

impl SecretRepository for SqlxSecretRepository {
    async fn get_by_id(&self, secret_id: Uuid) -> Result<Option<SecretItem>, AppError> {
        let row_opt = sqlx::query(
            "SELECT id, vault_id, secret_type, title, metadata_json, tags, expires_at, created_at, modified_at, usage_count, blob_storage, secret_blob, file_blob_ref, deleted_at
             FROM secret_items
             WHERE id = ?1 AND deleted_at IS NULL",
        )
        .bind(secret_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => Ok(Some(Self::row_to_secret_item(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_vault_id(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        let rows = sqlx::query(
              "SELECT id, vault_id, secret_type, title, metadata_json, tags, expires_at, created_at, modified_at, usage_count, blob_storage, secret_blob, file_blob_ref, deleted_at
             FROM secret_items
             WHERE vault_id = ?1 AND deleted_at IS NULL
               ORDER BY created_at DESC, id DESC",
        )
        .bind(vault_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(Self::row_to_secret_item(&row)?);
        }

        Ok(items)
    }

    async fn list_trash_by_vault_id(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        let rows = sqlx::query(
            "SELECT id, vault_id, secret_type, title, metadata_json, tags, expires_at, created_at, modified_at, usage_count, blob_storage, secret_blob, file_blob_ref, deleted_at
             FROM secret_items
             WHERE vault_id = ?1 AND deleted_at IS NOT NULL
             ORDER BY deleted_at DESC, id DESC",
        )
        .bind(vault_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(Self::row_to_secret_item(&row)?);
        }

        Ok(items)
    }

    async fn list_all_trash_by_owner_id(
        &self,
        owner_user_id: Uuid,
    ) -> Result<Vec<SecretItem>, AppError> {
        let rows = sqlx::query(
            "SELECT s.id, s.vault_id, s.secret_type, s.title, s.metadata_json, s.tags, s.expires_at, s.created_at, s.modified_at, s.usage_count, s.blob_storage, s.secret_blob, s.file_blob_ref, s.deleted_at
             FROM secret_items s
             INNER JOIN vaults v ON v.id = s.vault_id
             WHERE v.owner_user_id = ?1 AND s.deleted_at IS NOT NULL
             ORDER BY s.deleted_at DESC, s.id DESC",
        )
        .bind(owner_user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(Self::row_to_secret_item(&row)?);
        }

        Ok(items)
    }

    async fn insert_secret_blob(
        &self,
        item: &SecretItem,
        encrypted_secret_blob: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        let secret_blob = if matches!(item.blob_storage, BlobStorage::Inline) {
            Some(encrypted_secret_blob.expose_secret().as_slice())
        } else {
            None
        };
        let file_blob_ref = if matches!(item.blob_storage, BlobStorage::File) {
            Some(encrypted_secret_blob.expose_secret().as_slice())
        } else {
            None
        };

        sqlx::query(
            "INSERT INTO secret_items (
                id, vault_id, secret_type, title, metadata_json, tags, expires_at, blob_storage, secret_blob, file_blob_ref, deleted_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)",
        )
        .bind(item.id.to_string())
        .bind(item.vault_id.to_string())
        .bind(Self::to_secret_type_db(item.secret_type))
        .bind(item.title.as_deref())
        .bind(item.metadata_json.as_deref())
        .bind(item.tags.as_deref())
        .bind(item.expires_at.as_deref())
        .bind(Self::to_blob_storage_db(item.blob_storage))
        .bind(secret_blob)
        .bind(file_blob_ref)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_secret_metadata(
        &self,
        secret_id: Uuid,
        title: Option<String>,
        metadata_json: Option<String>,
        tags: Option<String>,
        expires_at: Option<String>,
    ) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE secret_items
             SET title = ?1,
                 metadata_json = ?2,
                 tags = ?3,
                 expires_at = ?4
             WHERE id = ?5 AND deleted_at IS NULL",
        )
        .bind(title)
        .bind(metadata_json)
        .bind(tags)
        .bind(expires_at)
        .bind(secret_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "secret not found for metadata update".to_string(),
            ));
        }

        Ok(())
    }

    async fn update_secret_blob(
        &self,
        secret_id: Uuid,
        encrypted_secret_blob: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        let row_opt = sqlx::query(
            "SELECT blob_storage
             FROM secret_items
             WHERE id = ?1 AND deleted_at IS NULL",
        )
        .bind(secret_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        let row = row_opt
            .ok_or_else(|| AppError::Storage("secret not found for blob update".to_string()))?;
        let blob_storage_raw: String = row.try_get("blob_storage")?;
        let blob_storage = Self::parse_blob_storage_db(&blob_storage_raw)?;

        let result = match blob_storage {
            BlobStorage::Inline => {
                sqlx::query(
                    "UPDATE secret_items
                     SET secret_blob = ?1, file_blob_ref = NULL
                     WHERE id = ?2 AND deleted_at IS NULL",
                )
                .bind(encrypted_secret_blob.expose_secret().as_slice())
                .bind(secret_id.to_string())
                .execute(&self.pool)
                .await
            }
            BlobStorage::File => {
                sqlx::query(
                    "UPDATE secret_items
                     SET file_blob_ref = ?1, secret_blob = NULL
                     WHERE id = ?2 AND deleted_at IS NULL",
                )
                .bind(encrypted_secret_blob.expose_secret().as_slice())
                .bind(secret_id.to_string())
                .execute(&self.pool)
                .await
            }
        }?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "secret not found for blob update".to_string(),
            ));
        }

        Ok(())
    }

    async fn increment_usage_count(&self, secret_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE secret_items
             SET usage_count = usage_count + 1, modified_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND deleted_at IS NULL",
        )
        .bind(secret_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "secret not found for usage count increment".to_string(),
            ));
        }

        Ok(())
    }

    async fn soft_delete(&self, secret_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE secret_items
             SET deleted_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND deleted_at IS NULL",
        )
        .bind(secret_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage("secret not found for delete".to_string()));
        }

        Ok(())
    }

    async fn restore_secret(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;

        let vault_deleted_at: Option<String> = sqlx::query_scalar(
            "SELECT deleted_at
             FROM vaults
             WHERE id = ?1",
        )
        .bind(vault_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Storage("vault not found for restore".to_string()))?;

        if vault_deleted_at.is_some() {
            sqlx::query(
                "UPDATE vaults
                 SET deleted_at = NULL
                 WHERE id = ?1 AND deleted_at IS NOT NULL",
            )
            .bind(vault_id.to_string())
            .execute(&mut *tx)
            .await?;
        }

        let result = sqlx::query(
            "UPDATE secret_items
             SET deleted_at = NULL
             WHERE id = ?1 AND vault_id = ?2 AND deleted_at IS NOT NULL",
        )
        .bind(secret_id.to_string())
        .bind(vault_id.to_string())
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage("secret not found in trash".to_string()));
        }

        tx.commit().await?;

        Ok(())
    }

    async fn permanent_delete(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            "DELETE FROM secret_items
             WHERE id = ?1 AND vault_id = ?2 AND deleted_at IS NOT NULL",
        )
        .bind(secret_id.to_string())
        .bind(vault_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "secret not found for permanent delete".to_string(),
            ));
        }

        Ok(())
    }

    async fn empty_trash(&self, vault_id: Uuid) -> Result<usize, AppError> {
        let result = sqlx::query(
            "DELETE FROM secret_items
             WHERE vault_id = ?1 AND deleted_at IS NOT NULL",
        )
        .bind(vault_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{SecretRepository, SqlxSecretRepository};
    use crate::errors::AppError;
    use crate::models::{BlobStorage, SecretItem, SecretType};
    use secrecy::{ExposeSecret, SecretBox};
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::Row;
    use uuid::Uuid;

    async fn setup_repo() -> Result<SqlxSecretRepository, String> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|err| format!("connect in-memory sqlite: {err}"))?;

        sqlx::query(
            "CREATE TABLE vaults (
                id TEXT PRIMARY KEY NOT NULL,
                owner_user_id TEXT,
                name TEXT,
                deleted_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create vaults table: {err}"))?;

        sqlx::query(
            "CREATE TABLE secret_items (
                id TEXT PRIMARY KEY NOT NULL,
                vault_id TEXT NOT NULL,
                secret_type TEXT NOT NULL,
                title TEXT,
                metadata_json TEXT,
                tags TEXT,
                expires_at TEXT,
                blob_storage TEXT NOT NULL,
                secret_blob BLOB,
                file_blob_ref BLOB,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                modified_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                usage_count INTEGER NOT NULL DEFAULT 0,
                deleted_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .map_err(|err| format!("create secret_items table: {err}"))?;

        Ok(SqlxSecretRepository::new(pool))
    }

    async fn insert_vault(
        repo: &SqlxSecretRepository,
        vault_id: Uuid,
        is_deleted: bool,
    ) -> Result<(), String> {
        let deleted_at = if is_deleted {
            Some("2026-01-01T00:00:00Z")
        } else {
            None
        };

        sqlx::query(
            "INSERT INTO vaults (id, owner_user_id, name, deleted_at)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(vault_id.to_string())
        .bind(Uuid::new_v4().to_string())
        .bind("Test Vault")
        .bind(deleted_at)
        .execute(&repo.pool)
        .await
        .map_err(|err| format!("insert vault row: {err}"))?;

        Ok(())
    }

    fn build_item(vault_id: Uuid, storage: BlobStorage) -> SecretItem {
        SecretItem {
            id: Uuid::new_v4(),
            vault_id,
            secret_type: SecretType::ApiToken,
            title: Some("Item de test".to_string()),
            metadata_json: None,
            tags: None,
            expires_at: None,
            created_at: None,
            modified_at: None,
            usage_count: 0,
            blob_storage: storage,
            secret_blob: SecretBox::new(Box::new(Vec::new())),
            deleted_at: None,
        }
    }

    #[tokio::test]
    async fn inline_strategy_roundtrip() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let item = build_item(vault_id, BlobStorage::Inline);
        let payload = SecretBox::new(Box::new(vec![1_u8, 2_u8, 3_u8]));

        let insert_result = repo.insert_secret_blob(&item, payload).await;
        assert!(insert_result.is_ok(), "insert should succeed");
        if insert_result.is_err() {
            return;
        }

        let found_result = repo.get_by_id(item.id).await;
        assert!(found_result.is_ok(), "get should succeed");
        let found_opt = match found_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(found_opt.is_some(), "secret should exist");
        let found = match found_opt {
            Some(value) => value,
            None => return,
        };

        assert!(matches!(found.blob_storage, BlobStorage::Inline));
        assert_eq!(
            found.secret_blob.expose_secret().as_slice(),
            &[1_u8, 2_u8, 3_u8]
        );
    }

    #[tokio::test]
    async fn file_strategy_roundtrip() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let item = build_item(vault_id, BlobStorage::File);
        let file_ref = SecretBox::new(Box::new(b"vault-a/item-1.bin".to_vec()));

        let insert_result = repo.insert_secret_blob(&item, file_ref).await;
        assert!(insert_result.is_ok(), "insert should succeed");
        if insert_result.is_err() {
            return;
        }

        let found_result = repo.get_by_id(item.id).await;
        assert!(found_result.is_ok(), "get should succeed");
        let found_opt = match found_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(found_opt.is_some(), "secret should exist");
        let found = match found_opt {
            Some(value) => value,
            None => return,
        };

        assert!(matches!(found.blob_storage, BlobStorage::File));
        assert_eq!(
            found.secret_blob.expose_secret().as_slice(),
            b"vault-a/item-1.bin"
        );
    }

    #[tokio::test]
    async fn list_by_vault_returns_both_strategies() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let inline_item = build_item(vault_id, BlobStorage::Inline);
        let file_item = build_item(vault_id, BlobStorage::File);

        let ins_a = repo
            .insert_secret_blob(&inline_item, SecretBox::new(Box::new(vec![7_u8, 7_u8])))
            .await;
        let ins_b = repo
            .insert_secret_blob(
                &file_item,
                SecretBox::new(Box::new(b"vault-a/item-2.bin".to_vec())),
            )
            .await;

        assert!(ins_a.is_ok(), "inline insert should succeed");
        assert!(ins_b.is_ok(), "file insert should succeed");
        if ins_a.is_err() || ins_b.is_err() {
            return;
        }

        let list_result = repo.list_by_vault_id(vault_id).await;
        assert!(list_result.is_ok(), "list should succeed");
        let items = match list_result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(items.len(), 2);
        let has_inline = items
            .iter()
            .any(|item| matches!(item.blob_storage, BlobStorage::Inline));
        let has_file = items
            .iter()
            .any(|item| matches!(item.blob_storage, BlobStorage::File));
        assert!(has_inline);
        assert!(has_file);
    }

    #[tokio::test]
    async fn update_blob_respects_storage_strategy() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let inline_item = build_item(vault_id, BlobStorage::Inline);
        let file_item = build_item(vault_id, BlobStorage::File);

        let ins_a = repo
            .insert_secret_blob(&inline_item, SecretBox::new(Box::new(vec![1_u8])))
            .await;
        let ins_b = repo
            .insert_secret_blob(&file_item, SecretBox::new(Box::new(b"old.bin".to_vec())))
            .await;
        assert!(ins_a.is_ok(), "inline seed should succeed");
        assert!(ins_b.is_ok(), "file seed should succeed");
        if ins_a.is_err() || ins_b.is_err() {
            return;
        }

        let up_a = repo
            .update_secret_blob(inline_item.id, SecretBox::new(Box::new(vec![9_u8, 9_u8])))
            .await;
        let up_b = repo
            .update_secret_blob(file_item.id, SecretBox::new(Box::new(b"new.bin".to_vec())))
            .await;
        assert!(up_a.is_ok(), "inline update should succeed");
        assert!(up_b.is_ok(), "file update should succeed");
        if up_a.is_err() || up_b.is_err() {
            return;
        }

        let inline_row_result =
            sqlx::query("SELECT secret_blob, file_blob_ref FROM secret_items WHERE id = ?1")
                .bind(inline_item.id.to_string())
                .fetch_one(&repo.pool)
                .await;
        assert!(inline_row_result.is_ok(), "inline readback should succeed");
        let inline_row = match inline_row_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let inline_blob_result: Result<Option<Vec<u8>>, _> = inline_row.try_get("secret_blob");
        let inline_ref_result: Result<Option<Vec<u8>>, _> = inline_row.try_get("file_blob_ref");
        assert!(inline_blob_result.is_ok());
        assert!(inline_ref_result.is_ok());
        let inline_blob = match inline_blob_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let inline_ref = match inline_ref_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(inline_blob, Some(vec![9_u8, 9_u8]));
        assert!(inline_ref.is_none());

        let file_row_result =
            sqlx::query("SELECT secret_blob, file_blob_ref FROM secret_items WHERE id = ?1")
                .bind(file_item.id.to_string())
                .fetch_one(&repo.pool)
                .await;
        assert!(file_row_result.is_ok(), "file readback should succeed");
        let file_row = match file_row_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let file_blob_result: Result<Option<Vec<u8>>, _> = file_row.try_get("secret_blob");
        let file_ref_result: Result<Option<Vec<u8>>, _> = file_row.try_get("file_blob_ref");
        assert!(file_blob_result.is_ok());
        assert!(file_ref_result.is_ok());
        let file_blob = match file_blob_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let file_ref = match file_ref_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(file_blob.is_none());
        assert_eq!(file_ref, Some(b"new.bin".to_vec()));
    }

    #[tokio::test]
    async fn update_metadata_persists_title_and_fields() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let item = build_item(vault_id, BlobStorage::Inline);
        let insert_result = repo
            .insert_secret_blob(&item, SecretBox::new(Box::new(vec![4_u8, 2_u8])))
            .await;
        assert!(insert_result.is_ok(), "insert should succeed");
        if insert_result.is_err() {
            return;
        }

        let update_result = repo
            .update_secret_metadata(
                item.id,
                Some("Titre modifie".to_string()),
                Some("{\"category\":\"Infra\"}".to_string()),
                Some("prod,urgent".to_string()),
                Some("2026-12-24T00:00:00Z".to_string()),
            )
            .await;
        assert!(update_result.is_ok(), "metadata update should succeed");
        if update_result.is_err() {
            return;
        }

        let found_result = repo.get_by_id(item.id).await;
        assert!(found_result.is_ok(), "get should succeed");
        let found = match found_result {
            Ok(Some(value)) => value,
            _ => return,
        };

        assert_eq!(found.title.as_deref(), Some("Titre modifie"));
        assert_eq!(
            found.metadata_json.as_deref(),
            Some("{\"category\":\"Infra\"}")
        );
        assert_eq!(found.tags.as_deref(), Some("prod,urgent"));
        assert_eq!(found.expires_at.as_deref(), Some("2026-12-24T00:00:00Z"));
    }

    #[tokio::test]
    async fn soft_delete_hides_items() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let item = build_item(vault_id, BlobStorage::Inline);
        let insert_result = repo
            .insert_secret_blob(&item, SecretBox::new(Box::new(vec![5_u8, 5_u8, 5_u8])))
            .await;
        assert!(insert_result.is_ok(), "insert should succeed");
        if insert_result.is_err() {
            return;
        }

        let delete_result = repo.soft_delete(item.id).await;
        assert!(delete_result.is_ok(), "delete should succeed");
        if delete_result.is_err() {
            return;
        }

        let by_id_result = repo.get_by_id(item.id).await;
        assert!(by_id_result.is_ok(), "get_by_id should succeed");
        let by_id = match by_id_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(by_id.is_none());

        let list_result = repo.list_by_vault_id(vault_id).await;
        assert!(list_result.is_ok(), "list should succeed");
        let list = match list_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn trash_restore_and_permanent_delete_work() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let insert_vault_result = insert_vault(&repo, vault_id, false).await;
        assert!(insert_vault_result.is_ok(), "vault insert should succeed");
        if insert_vault_result.is_err() {
            return;
        }

        let item = build_item(vault_id, BlobStorage::Inline);
        let insert_result = repo
            .insert_secret_blob(&item, SecretBox::new(Box::new(vec![8_u8, 8_u8])))
            .await;
        assert!(insert_result.is_ok(), "insert should succeed");
        if insert_result.is_err() {
            return;
        }

        let delete_result = repo.soft_delete(item.id).await;
        assert!(delete_result.is_ok(), "soft delete should succeed");
        if delete_result.is_err() {
            return;
        }

        let trash_result = repo.list_trash_by_vault_id(vault_id).await;
        assert!(trash_result.is_ok(), "trash list should succeed");
        let trash_items = match trash_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(trash_items.len(), 1);
        assert_eq!(trash_items[0].id, item.id);

        let restore_result = repo.restore_secret(item.id, vault_id).await;
        assert!(restore_result.is_ok(), "restore should succeed");
        if restore_result.is_err() {
            return;
        }

        let active_result = repo.list_by_vault_id(vault_id).await;
        assert!(active_result.is_ok(), "active list should succeed");
        let active_items = match active_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(active_items.len(), 1);

        let second_delete_result = repo.soft_delete(item.id).await;
        assert!(
            second_delete_result.is_ok(),
            "second soft delete should succeed"
        );
        if second_delete_result.is_err() {
            return;
        }

        let permanent_result = repo.permanent_delete(item.id, vault_id).await;
        assert!(permanent_result.is_ok(), "permanent delete should succeed");
        if permanent_result.is_err() {
            return;
        }

        let by_id_result = repo.get_by_id(item.id).await;
        assert!(by_id_result.is_ok(), "get by id should succeed");
        let by_id = match by_id_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(by_id.is_none());
    }

    #[tokio::test]
    async fn restore_secret_auto_restores_deleted_vault() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_id = Uuid::new_v4();
        let insert_vault_result = insert_vault(&repo, vault_id, true).await;
        assert!(insert_vault_result.is_ok(), "vault insert should succeed");
        if insert_vault_result.is_err() {
            return;
        }

        let item = build_item(vault_id, BlobStorage::Inline);
        let insert_result = repo
            .insert_secret_blob(&item, SecretBox::new(Box::new(vec![9_u8, 9_u8])))
            .await;
        assert!(insert_result.is_ok(), "insert should succeed");
        if insert_result.is_err() {
            return;
        }

        let delete_result = repo.soft_delete(item.id).await;
        assert!(delete_result.is_ok(), "soft delete should succeed");
        if delete_result.is_err() {
            return;
        }

        let restore_result = repo.restore_secret(item.id, vault_id).await;
        assert!(restore_result.is_ok(), "restore should succeed");
        if restore_result.is_err() {
            return;
        }

        let vault_state_result = sqlx::query("SELECT deleted_at FROM vaults WHERE id = ?1")
            .bind(vault_id.to_string())
            .fetch_optional(&repo.pool)
            .await;
        assert!(vault_state_result.is_ok(), "vault lookup should succeed");
        let vault_row_opt = match vault_state_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(vault_row_opt.is_some(), "vault should exist");
        let vault_row = match vault_row_opt {
            Some(value) => value,
            None => return,
        };
        let deleted_at: Option<String> = vault_row
            .try_get("deleted_at")
            .unwrap_or(Some("unexpected".to_string()));
        assert!(deleted_at.is_none(), "vault should be restored");

        let active_result = repo.list_by_vault_id(vault_id).await;
        assert!(active_result.is_ok(), "active list should succeed");
        let active_items = match active_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(active_items.len(), 1);
        assert_eq!(active_items[0].id, item.id);
    }

    #[tokio::test]
    async fn empty_trash_is_scoped_to_vault() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let vault_a = Uuid::new_v4();
        let vault_b = Uuid::new_v4();
        let item_a = build_item(vault_a, BlobStorage::Inline);
        let item_b = build_item(vault_b, BlobStorage::Inline);

        let ins_a = repo
            .insert_secret_blob(&item_a, SecretBox::new(Box::new(vec![1_u8])))
            .await;
        let ins_b = repo
            .insert_secret_blob(&item_b, SecretBox::new(Box::new(vec![2_u8])))
            .await;
        assert!(ins_a.is_ok() && ins_b.is_ok(), "insert should succeed");
        if ins_a.is_err() || ins_b.is_err() {
            return;
        }

        let del_a = repo.soft_delete(item_a.id).await;
        let del_b = repo.soft_delete(item_b.id).await;
        assert!(del_a.is_ok() && del_b.is_ok(), "soft delete should succeed");
        if del_a.is_err() || del_b.is_err() {
            return;
        }

        let emptied_result = repo.empty_trash(vault_a).await;
        assert!(emptied_result.is_ok(), "empty trash should succeed");
        let emptied = match emptied_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(emptied, 1);

        let remaining_a_result = repo.list_trash_by_vault_id(vault_a).await;
        assert!(
            remaining_a_result.is_ok(),
            "vault a trash list should succeed"
        );
        let remaining_a = match remaining_a_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(remaining_a.is_empty());

        let remaining_b_result = repo.list_trash_by_vault_id(vault_b).await;
        assert!(
            remaining_b_result.is_ok(),
            "vault b trash list should succeed"
        );
        let remaining_b = match remaining_b_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(remaining_b.len(), 1);
        assert_eq!(remaining_b[0].id, item_b.id);
    }

    #[tokio::test]
    async fn missing_item_operations_return_storage_error() {
        let repo_result = setup_repo().await;
        assert!(repo_result.is_ok(), "repo setup should succeed");
        let repo = match repo_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let missing_id = Uuid::new_v4();
        let update_result = repo
            .update_secret_blob(missing_id, SecretBox::new(Box::new(vec![1_u8])))
            .await;
        assert!(
            update_result.is_err(),
            "update should fail for missing item"
        );
        if let Err(err) = update_result {
            assert!(matches!(err, AppError::Storage(_)));
        }

        let delete_result = repo.soft_delete(missing_id).await;
        assert!(
            delete_result.is_err(),
            "delete should fail for missing item"
        );
        if let Err(err) = delete_result {
            assert!(matches!(err, AppError::Storage(_)));
        }
    }
}

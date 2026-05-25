use std::sync::Arc;

use secrecy::{ExposeSecret, SecretBox};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AuditAction, BlobStorage, SecretItem, SecretType};
use crate::repositories::secret_repository::SecretRepository;
use crate::services::audit_log_service::AuditLogService;
use crate::services::crypto_service::{CryptoService, EncryptedPayload, NONCE_LEN};

#[derive(Debug)]
pub struct DecryptedSecret {
    /// Secret identifier.
    pub id: Uuid,
    /// Parent vault identifier.
    pub vault_id: Uuid,
    /// Secret domain type (password/token/key/document).
    pub secret_type: SecretType,
    /// Backing storage mode (inline/file).
    pub blob_storage: BlobStorage,
    /// Decrypted secret payload.
    pub secret_value: SecretBox<Vec<u8>>,
}

#[trait_variant::make(SecretService: Send)]
pub trait LocalSecretService {
    /// Create and encrypt a new secret item inside a vault.
    ///
    /// `plaintext_secret` and `vault_key` are sensitive values that must be
    /// handled as short-lived material by callers.
    #[allow(clippy::too_many_arguments)]
    async fn create_secret(
        &self,
        vault_id: Uuid,
        secret_type: SecretType,
        title: Option<String>,
        metadata_json: Option<String>,
        tags: Option<String>,
        expires_at: Option<String>,
        plaintext_secret: SecretBox<Vec<u8>>,
        vault_key: SecretBox<Vec<u8>>,
    ) -> Result<SecretItem, AppError>;
    /// Read and decrypt a secret using the caller-provided vault key.
    async fn get_secret(
        &self,
        secret_id: Uuid,
        vault_key: SecretBox<Vec<u8>>,
    ) -> Result<DecryptedSecret, AppError>;
    /// Update secret metadata and optionally rotate encrypted payload.
    ///
    /// `audit_detail` is an optional JSON string that will be stored in the
    /// audit log alongside the `secret.updated` event (e.g. title and which
    /// fields changed).  Pass `None` when the context is unavailable.
    #[allow(clippy::too_many_arguments)]
    async fn update_secret(
        &self,
        secret_id: Uuid,
        title: Option<String>,
        metadata_json: Option<String>,
        tags: Option<String>,
        expires_at: Option<String>,
        plaintext_secret: Option<SecretBox<Vec<u8>>>,
        vault_key: SecretBox<Vec<u8>>,
        audit_detail: Option<String>,
    ) -> Result<(), AppError>;
    /// Record a `secret.viewed` event for CNIL-compliant access logging.
    async fn record_viewed(
        &self,
        secret_id: Uuid,
        actor_id: Option<Uuid>,
        title: Option<&str>,
    ) -> Result<(), AppError>;
    /// Record a `secret.password_copied` or `secret.field_copied` event.
    async fn record_field_copy(
        &self,
        secret_id: Uuid,
        actor_id: Option<Uuid>,
        title: Option<&str>,
        field: &str,
    ) -> Result<(), AppError>;
    /// Move one secret to another vault (with payload re-encryption).
    async fn move_secret(
        &self,
        secret_id: Uuid,
        target_vault_id: Uuid,
        source_vault_key: SecretBox<Vec<u8>>,
        target_vault_key: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    /// List non-deleted secrets for one vault.
    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError>;
    /// List trashed secrets for one vault.
    async fn list_trash_by_vault(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError>;
    /// List all trashed secrets visible to a user across vaults.
    async fn list_all_trash_by_user(&self, user_id: Uuid) -> Result<Vec<SecretItem>, AppError>;
    /// Soft-delete a secret (move to trash).
    async fn soft_delete(&self, secret_id: Uuid) -> Result<(), AppError>;
    /// Restore a trashed secret back to a vault.
    async fn restore_secret(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError>;
    /// Permanently delete one secret.
    async fn permanent_delete(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError>;
    /// Permanently delete all trashed secrets in a vault.
    async fn empty_trash(&self, vault_id: Uuid) -> Result<usize, AppError>;
    /// Increase usage metrics for one secret (telemetry/security UX).
    async fn increment_usage_count(&self, secret_id: Uuid) -> Result<(), AppError>;
}

pub struct SecretServiceImpl<TRepo, TCrypto, TAuditSvc>
where
    TRepo: SecretRepository + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    secret_repo: TRepo,
    crypto_service: TCrypto,
    audit_service: Arc<TAuditSvc>,
}

impl<TRepo, TCrypto, TAuditSvc> SecretServiceImpl<TRepo, TCrypto, TAuditSvc>
where
    TRepo: SecretRepository + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    pub fn new(secret_repo: TRepo, crypto_service: TCrypto, audit_service: Arc<TAuditSvc>) -> Self {
        Self {
            secret_repo,
            crypto_service,
            audit_service,
        }
    }

    fn blob_storage_for_type(secret_type: SecretType) -> BlobStorage {
        match secret_type {
            SecretType::Password | SecretType::ApiToken | SecretType::SshKey => BlobStorage::Inline,
            SecretType::SecureDocument => BlobStorage::File,
        }
    }

    fn serialize_payload(payload: &EncryptedPayload) -> SecretBox<Vec<u8>> {
        let mut bytes = Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
        bytes.extend_from_slice(&payload.nonce);
        bytes.extend_from_slice(payload.ciphertext.expose_secret().as_slice());
        SecretBox::new(Box::new(bytes))
    }

    fn deserialize_payload(bytes: &SecretBox<Vec<u8>>) -> Result<EncryptedPayload, AppError> {
        if bytes.expose_secret().len() < NONCE_LEN {
            return Err(AppError::Storage(
                "secret blob envelope is invalid".to_string(),
            ));
        }

        let mut nonce = [0_u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes.expose_secret()[0..NONCE_LEN]);
        let ciphertext = bytes.expose_secret()[NONCE_LEN..].to_vec();

        Ok(EncryptedPayload {
            nonce,
            ciphertext: SecretBox::new(Box::new(ciphertext)),
        })
    }

    fn validate_plaintext_secret(plaintext_secret: &SecretBox<Vec<u8>>) -> Result<(), AppError> {
        if plaintext_secret.expose_secret().is_empty() {
            return Err(AppError::Validation(
                "secret value must not be empty".to_string(),
            ));
        }

        Ok(())
    }
}

impl<TRepo, TCrypto, TAuditSvc> SecretService for SecretServiceImpl<TRepo, TCrypto, TAuditSvc>
where
    TRepo: SecretRepository + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    async fn create_secret(
        &self,
        vault_id: Uuid,
        secret_type: SecretType,
        title: Option<String>,
        metadata_json: Option<String>,
        tags: Option<String>,
        expires_at: Option<String>,
        plaintext_secret: SecretBox<Vec<u8>>,
        vault_key: SecretBox<Vec<u8>>,
    ) -> Result<SecretItem, AppError> {
        Self::validate_plaintext_secret(&plaintext_secret)?;

        let blob_storage = Self::blob_storage_for_type(secret_type);
        let encrypted_payload = self
            .crypto_service
            .encrypt(&plaintext_secret, &vault_key)
            .await?;
        let serialized_payload = Self::serialize_payload(&encrypted_payload);
        let stored_blob = SecretBox::new(Box::new(serialized_payload.expose_secret().clone()));

        let item = SecretItem {
            id: Uuid::new_v4(),
            vault_id,
            secret_type,
            title,
            metadata_json,
            tags,
            expires_at,
            created_at: None,
            modified_at: None,
            usage_count: 0,
            blob_storage,
            secret_blob: stored_blob,
            deleted_at: None,
        };

        self.secret_repo
            .insert_secret_blob(&item, serialized_payload)
            .await?;

        let secret_title_detail = format!(
            r#"{{"title":"{}"}}"#,
            item.title.as_deref().unwrap_or("Sans titre")
        );

        self.audit_service
            .record_event(
                None,
                AuditAction::SecretCreated,
                Some("secret"),
                Some(&item.id.to_string()),
                Some(secret_title_detail.as_str()),
            )
            .await
            .ok();

        Ok(item)
    }

    async fn get_secret(
        &self,
        secret_id: Uuid,
        vault_key: SecretBox<Vec<u8>>,
    ) -> Result<DecryptedSecret, AppError> {
        let item = self
            .secret_repo
            .get_by_id(secret_id)
            .await?
            .ok_or_else(|| AppError::NotFound("secret not found".to_string()))?;

        let payload = Self::deserialize_payload(&item.secret_blob)?;
        let secret_value = self.crypto_service.decrypt(&payload, &vault_key).await?;

        Ok(DecryptedSecret {
            id: item.id,
            vault_id: item.vault_id,
            secret_type: item.secret_type,
            blob_storage: item.blob_storage,
            secret_value,
        })
    }

    async fn update_secret(
        &self,
        secret_id: Uuid,
        title: Option<String>,
        metadata_json: Option<String>,
        tags: Option<String>,
        expires_at: Option<String>,
        plaintext_secret: Option<SecretBox<Vec<u8>>>,
        vault_key: SecretBox<Vec<u8>>,
        audit_detail: Option<String>,
    ) -> Result<(), AppError> {
        self.secret_repo
            .update_secret_metadata(secret_id, title, metadata_json, tags, expires_at)
            .await?;

        if let Some(plaintext) = plaintext_secret {
            Self::validate_plaintext_secret(&plaintext)?;
            let encrypted_payload = self.crypto_service.encrypt(&plaintext, &vault_key).await?;
            let serialized_payload = Self::serialize_payload(&encrypted_payload);
            self.secret_repo
                .update_secret_blob(secret_id, serialized_payload)
                .await?;
        }

        self.audit_service
            .record_event(
                None,
                AuditAction::SecretUpdated,
                Some("secret"),
                Some(&secret_id.to_string()),
                audit_detail.as_deref(),
            )
            .await
            .ok();

        Ok(())
    }

    async fn record_viewed(
        &self,
        secret_id: Uuid,
        actor_id: Option<Uuid>,
        title: Option<&str>,
    ) -> Result<(), AppError> {
        let detail = title.map(|t| format!(r#"{{"title":"{t}"}}"#));
        self.audit_service
            .record_event(
                actor_id,
                AuditAction::SecretViewed,
                Some("secret"),
                Some(&secret_id.to_string()),
                detail.as_deref(),
            )
            .await
            .ok();
        Ok(())
    }

    async fn record_field_copy(
        &self,
        secret_id: Uuid,
        actor_id: Option<Uuid>,
        title: Option<&str>,
        field: &str,
    ) -> Result<(), AppError> {
        let action = if field == "password" {
            AuditAction::SecretPasswordCopied
        } else {
            AuditAction::SecretFieldCopied
        };
        let detail = title.map(|t| format!(r#"{{"title":"{t}","field":"{field}"}}"#));
        self.audit_service
            .record_event(
                actor_id,
                action,
                Some("secret"),
                Some(&secret_id.to_string()),
                detail.as_deref(),
            )
            .await
            .ok();
        Ok(())
    }

    async fn move_secret(
        &self,
        secret_id: Uuid,
        target_vault_id: Uuid,
        source_vault_key: SecretBox<Vec<u8>>,
        target_vault_key: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        let item = self
            .secret_repo
            .get_by_id(secret_id)
            .await?
            .ok_or_else(|| AppError::NotFound("secret not found".to_string()))?;

        if item.vault_id == target_vault_id {
            return Ok(());
        }

        let payload = Self::deserialize_payload(&item.secret_blob)?;
        let plaintext = self
            .crypto_service
            .decrypt(&payload, &source_vault_key)
            .await?;
        let encrypted_for_target = self
            .crypto_service
            .encrypt(&plaintext, &target_vault_key)
            .await?;
        let serialized_payload = Self::serialize_payload(&encrypted_for_target);

        self.secret_repo
            .move_secret_to_vault(secret_id, target_vault_id, serialized_payload)
            .await
    }

    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        self.secret_repo.list_by_vault_id(vault_id).await
    }

    async fn list_trash_by_vault(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        self.secret_repo.list_trash_by_vault_id(vault_id).await
    }

    async fn list_all_trash_by_user(&self, user_id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        self.secret_repo.list_all_trash_by_owner_id(user_id).await
    }

    async fn soft_delete(&self, secret_id: Uuid) -> Result<(), AppError> {
        self.secret_repo.soft_delete(secret_id).await?;
        self.audit_service
            .record_event(
                None,
                AuditAction::SecretDeleted,
                Some("secret"),
                Some(&secret_id.to_string()),
                Some("soft"),
            )
            .await
            .ok();
        Ok(())
    }

    async fn restore_secret(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError> {
        self.secret_repo.restore_secret(secret_id, vault_id).await
    }

    async fn permanent_delete(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError> {
        self.secret_repo
            .permanent_delete(secret_id, vault_id)
            .await?;
        self.audit_service
            .record_event(
                None,
                AuditAction::SecretDeleted,
                Some("secret"),
                Some(&secret_id.to_string()),
                Some("permanent"),
            )
            .await
            .ok();
        Ok(())
    }

    async fn empty_trash(&self, vault_id: Uuid) -> Result<usize, AppError> {
        self.secret_repo.empty_trash(vault_id).await
    }

    async fn increment_usage_count(&self, secret_id: Uuid) -> Result<(), AppError> {
        self.secret_repo.increment_usage_count(secret_id).await
    }
}

impl<TRepo, TCrypto, TAuditSvc> SecretServiceImpl<TRepo, TCrypto, TAuditSvc>
where
    TRepo: SecretRepository + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    /// Évalue la robustesse d'un mot de passe basée sur sa longueur et sa complexité
    pub fn evaluate_password_strength(secret_value: &[u8]) -> String {
        // Minimum: 12 caractères, avec au moins 3 types de caractères différents
        if secret_value.len() >= 12 {
            let content = String::from_utf8_lossy(secret_value);
            let has_uppercase = content.chars().any(|c| c.is_uppercase());
            let has_lowercase = content.chars().any(|c| c.is_lowercase());
            let has_digit = content.chars().any(|c| c.is_numeric());
            let has_special = content.chars().any(|c| !c.is_alphanumeric());

            let complexity = [has_uppercase, has_lowercase, has_digit, has_special]
                .iter()
                .filter(|&&b| b)
                .count();

            if complexity >= 3 {
                "Robuste".to_string()
            } else {
                "Faible".to_string()
            }
        } else {
            "Faible".to_string()
        }
    }

    /// Trouve les IDs des secrets qui partagent la même valeur (doublons potentiels)
    pub async fn find_duplicate_secrets(
        &self,
        vault_id: Uuid,
        secret_value: &[u8],
    ) -> Result<Vec<Uuid>, AppError> {
        let all_items = self.secret_repo.list_by_vault_id(vault_id).await?;
        let mut duplicates = Vec::new();

        for item in all_items {
            // Pour cette version simplifiée, on compare le blob secret
            // Une vraie comparaison nécessiterait le déchiffrement avec vault key
            if item.secret_blob.expose_secret() == secret_value {
                duplicates.push(item.id);
            }
        }

        Ok(duplicates)
    }

    /// Incrémente le compteur d'utilisation de ce secret
    pub async fn increment_secret_usage_count(&self, secret_id: Uuid) -> Result<(), AppError> {
        self.secret_repo.increment_usage_count(secret_id).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{DecryptedSecret, SecretService, SecretServiceImpl};
    use crate::errors::AppError;
    use crate::models::{AuditAction, AuditLogEntry, BlobStorage, SecretItem, SecretType};
    use crate::repositories::secret_repository::SecretRepository;
    use crate::services::audit_log_service::AuditLogService;
    use crate::services::crypto_service::{CryptoService, EncryptedPayload, NONCE_LEN};
    use secrecy::{ExposeSecret, SecretBox, SecretString};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    struct StoredSecretRecord {
        id: Uuid,
        vault_id: Uuid,
        secret_type: SecretType,
        title: Option<String>,
        metadata_json: Option<String>,
        tags: Option<String>,
        expires_at: Option<String>,
        blob_storage: BlobStorage,
        blob: Vec<u8>,
        deleted: bool,
    }

    #[derive(Default, Clone)]
    struct StubSecretRepository {
        items: Arc<Mutex<HashMap<Uuid, StoredSecretRecord>>>,
    }

    impl StubSecretRepository {
        fn lock_items(
            &self,
        ) -> Result<std::sync::MutexGuard<'_, HashMap<Uuid, StoredSecretRecord>>, AppError>
        {
            self.items
                .lock()
                .map_err(|_| AppError::Storage("secret repository lock poisoned".to_string()))
        }
    }

    impl SecretRepository for StubSecretRepository {
        async fn get_by_id(&self, secret_id: Uuid) -> Result<Option<SecretItem>, AppError> {
            let items = self.lock_items()?;
            let item_opt = items.get(&secret_id);

            match item_opt {
                Some(item) if !item.deleted => Ok(Some(SecretItem {
                    id: item.id,
                    vault_id: item.vault_id,
                    secret_type: item.secret_type,
                    title: item.title.clone(),
                    metadata_json: item.metadata_json.clone(),
                    tags: item.tags.clone(),
                    expires_at: item.expires_at.clone(),
                    created_at: None,
                    modified_at: None,
                    usage_count: 0,
                    blob_storage: item.blob_storage,
                    secret_blob: SecretBox::new(Box::new(item.blob.clone())),
                    deleted_at: None,
                })),
                Some(_) => Ok(None),
                None => Ok(None),
            }
        }

        async fn list_by_vault_id(&self, vault_id: Uuid) -> Result<Vec<SecretItem>, AppError> {
            let items = self.lock_items()?;
            let listed = items
                .values()
                .filter(|item| item.vault_id == vault_id && !item.deleted)
                .map(|item| SecretItem {
                    id: item.id,
                    vault_id: item.vault_id,
                    secret_type: item.secret_type,
                    title: item.title.clone(),
                    metadata_json: item.metadata_json.clone(),
                    tags: item.tags.clone(),
                    expires_at: item.expires_at.clone(),
                    created_at: None,
                    modified_at: None,
                    usage_count: 0,
                    blob_storage: item.blob_storage,
                    secret_blob: SecretBox::new(Box::new(item.blob.clone())),
                    deleted_at: None,
                })
                .collect();

            Ok(listed)
        }

        async fn list_trash_by_vault_id(
            &self,
            vault_id: Uuid,
        ) -> Result<Vec<SecretItem>, AppError> {
            let items = self.lock_items()?;
            let listed = items
                .values()
                .filter(|item| item.vault_id == vault_id && item.deleted)
                .map(|item| SecretItem {
                    id: item.id,
                    vault_id: item.vault_id,
                    secret_type: item.secret_type,
                    title: item.title.clone(),
                    metadata_json: item.metadata_json.clone(),
                    tags: item.tags.clone(),
                    expires_at: item.expires_at.clone(),
                    created_at: None,
                    modified_at: None,
                    usage_count: 0,
                    blob_storage: item.blob_storage,
                    secret_blob: SecretBox::new(Box::new(item.blob.clone())),
                    deleted_at: None,
                })
                .collect();

            Ok(listed)
        }

        async fn insert_secret_blob(
            &self,
            item: &SecretItem,
            encrypted_secret_blob: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            let mut items = self.lock_items()?;
            items.insert(
                item.id,
                StoredSecretRecord {
                    id: item.id,
                    vault_id: item.vault_id,
                    secret_type: item.secret_type,
                    title: item.title.clone(),
                    metadata_json: item.metadata_json.clone(),
                    tags: item.tags.clone(),
                    expires_at: item.expires_at.clone(),
                    blob_storage: item.blob_storage,
                    blob: encrypted_secret_blob.expose_secret().clone(),
                    deleted: false,
                },
            );
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
            let mut items = self.lock_items()?;
            let item = items.get_mut(&secret_id).ok_or_else(|| {
                AppError::Storage("secret not found for metadata update".to_string())
            })?;
            if item.deleted {
                return Err(AppError::Storage(
                    "secret not found for metadata update".to_string(),
                ));
            }
            item.title = title;
            item.metadata_json = metadata_json;
            item.tags = tags;
            item.expires_at = expires_at;
            Ok(())
        }

        async fn update_secret_blob(
            &self,
            secret_id: Uuid,
            encrypted_secret_blob: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            let mut items = self.lock_items()?;
            let item = items
                .get_mut(&secret_id)
                .ok_or_else(|| AppError::Storage("secret not found for update".to_string()))?;
            item.blob = encrypted_secret_blob.expose_secret().clone();
            Ok(())
        }

        async fn move_secret_to_vault(
            &self,
            secret_id: Uuid,
            target_vault_id: Uuid,
            encrypted_secret_blob: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            let mut items = self.lock_items()?;
            let item = items
                .get_mut(&secret_id)
                .ok_or_else(|| AppError::Storage("secret not found for move".to_string()))?;
            if item.deleted {
                return Err(AppError::Storage("secret not found for move".to_string()));
            }
            item.vault_id = target_vault_id;
            item.blob = encrypted_secret_blob.expose_secret().clone();
            Ok(())
        }

        async fn soft_delete(&self, secret_id: Uuid) -> Result<(), AppError> {
            let mut items = self.lock_items()?;
            let item = items
                .get_mut(&secret_id)
                .ok_or_else(|| AppError::Storage("secret not found for delete".to_string()))?;
            item.deleted = true;
            Ok(())
        }

        async fn restore_secret(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError> {
            let mut items = self.lock_items()?;
            let item = items
                .get_mut(&secret_id)
                .ok_or_else(|| AppError::Storage("secret not found in trash".to_string()))?;
            if item.vault_id != vault_id || !item.deleted {
                return Err(AppError::Storage("secret not found in trash".to_string()));
            }
            item.deleted = false;
            Ok(())
        }

        async fn permanent_delete(&self, secret_id: Uuid, vault_id: Uuid) -> Result<(), AppError> {
            let mut items = self.lock_items()?;
            let can_delete = items
                .get(&secret_id)
                .map(|item| item.vault_id == vault_id && item.deleted)
                .unwrap_or(false);

            if !can_delete {
                return Err(AppError::Storage(
                    "secret not found for permanent delete".to_string(),
                ));
            }

            items.remove(&secret_id);
            Ok(())
        }

        async fn empty_trash(&self, vault_id: Uuid) -> Result<usize, AppError> {
            let mut items = self.lock_items()?;
            let before = items.len();
            items.retain(|_, item| !(item.vault_id == vault_id && item.deleted));
            Ok(before.saturating_sub(items.len()))
        }

        async fn list_all_trash_by_owner_id(
            &self,
            _owner_user_id: Uuid,
        ) -> Result<Vec<SecretItem>, AppError> {
            let items = self.lock_items()?;
            let listed = items
                .values()
                .filter(|item| item.deleted)
                .map(|item| SecretItem {
                    id: item.id,
                    vault_id: item.vault_id,
                    secret_type: item.secret_type,
                    title: item.title.clone(),
                    metadata_json: item.metadata_json.clone(),
                    tags: item.tags.clone(),
                    expires_at: item.expires_at.clone(),
                    created_at: None,
                    modified_at: None,
                    usage_count: 0,
                    blob_storage: item.blob_storage,
                    secret_blob: SecretBox::new(Box::new(item.blob.clone())),
                    deleted_at: None,
                })
                .collect();
            Ok(listed)
        }

        async fn increment_usage_count(&self, _secret_id: Uuid) -> Result<(), AppError> {
            // Stub implementation: no-op
            Ok(())
        }
    }

    #[derive(Default, Clone)]
    struct StubCryptoService;

    impl CryptoService for StubCryptoService {
        async fn generate_kdf_salt(&self) -> Result<SecretBox<Vec<u8>>, AppError> {
            Ok(SecretBox::new(Box::new(vec![1_u8; 32])))
        }

        async fn derive_key(
            &self,
            _password: &SecretString,
            _salt: &SecretBox<Vec<u8>>,
        ) -> Result<SecretBox<Vec<u8>>, AppError> {
            Ok(SecretBox::new(Box::new(vec![2_u8; 32])))
        }

        async fn encrypt(
            &self,
            plaintext: &SecretBox<Vec<u8>>,
            _key: &SecretBox<Vec<u8>>,
        ) -> Result<EncryptedPayload, AppError> {
            let mut nonce = [0_u8; NONCE_LEN];
            nonce[0] = 17;

            let mut ciphertext = plaintext.expose_secret().clone();
            ciphertext.reverse();

            Ok(EncryptedPayload {
                nonce,
                ciphertext: SecretBox::new(Box::new(ciphertext)),
            })
        }

        async fn decrypt(
            &self,
            payload: &EncryptedPayload,
            _key: &SecretBox<Vec<u8>>,
        ) -> Result<SecretBox<Vec<u8>>, AppError> {
            let mut plaintext = payload.ciphertext.expose_secret().clone();
            plaintext.reverse();
            Ok(SecretBox::new(Box::new(plaintext)))
        }
    }

    #[derive(Default, Clone)]
    struct StubAuditLogService;

    impl AuditLogService for StubAuditLogService {
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

    async fn assert_secret_roundtrip(
        secret_type: SecretType,
        plaintext: &[u8],
    ) -> Result<DecryptedSecret, AppError> {
        let repo = StubSecretRepository::default();
        let service = SecretServiceImpl::new(
            repo.clone(),
            StubCryptoService,
            Arc::new(StubAuditLogService),
        );
        let vault_id = Uuid::new_v4();
        let vault_key = SecretBox::new(Box::new(vec![7_u8; 32]));

        let created = service
            .create_secret(
                vault_id,
                secret_type,
                Some("Titre de test".to_string()),
                None,
                None,
                None,
                SecretBox::new(Box::new(plaintext.to_vec())),
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            )
            .await?;

        match secret_type {
            SecretType::Password | SecretType::ApiToken | SecretType::SshKey => {
                if !matches!(created.blob_storage, BlobStorage::Inline) {
                    return Err(AppError::Internal);
                }
            }
            SecretType::SecureDocument => {
                if !matches!(created.blob_storage, BlobStorage::File) {
                    return Err(AppError::Internal);
                }
            }
        }

        if created.secret_blob.expose_secret().len() <= NONCE_LEN {
            return Err(AppError::Internal);
        }

        service.get_secret(created.id, vault_key).await
    }

    #[tokio::test]
    async fn create_and_get_password_secret() {
        let result = assert_secret_roundtrip(SecretType::Password, b"super-secret-password").await;
        assert!(result.is_ok(), "password roundtrip should succeed");
        let decrypted = match result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert!(matches!(decrypted.secret_type, SecretType::Password));
        assert!(matches!(decrypted.blob_storage, BlobStorage::Inline));
        assert_eq!(
            decrypted.secret_value.expose_secret().as_slice(),
            b"super-secret-password"
        );
    }

    #[tokio::test]
    async fn create_and_get_api_token_secret() {
        let result = assert_secret_roundtrip(SecretType::ApiToken, b"token-abc-123").await;
        assert!(result.is_ok(), "api token roundtrip should succeed");
        let decrypted = match result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert!(matches!(decrypted.secret_type, SecretType::ApiToken));
        assert!(matches!(decrypted.blob_storage, BlobStorage::Inline));
        assert_eq!(
            decrypted.secret_value.expose_secret().as_slice(),
            b"token-abc-123"
        );
    }

    #[tokio::test]
    async fn create_and_get_ssh_key_secret() {
        let result = assert_secret_roundtrip(
            SecretType::SshKey,
            b"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEexample",
        )
        .await;
        assert!(result.is_ok(), "ssh key roundtrip should succeed");
        let decrypted = match result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert!(matches!(decrypted.secret_type, SecretType::SshKey));
        assert!(matches!(decrypted.blob_storage, BlobStorage::Inline));
        assert_eq!(
            decrypted.secret_value.expose_secret().as_slice(),
            b"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEexample"
        );
    }

    #[tokio::test]
    async fn list_by_vault_and_soft_delete_excludes_deleted_secret() {
        let repo = StubSecretRepository::default();
        let service =
            SecretServiceImpl::new(repo, StubCryptoService, Arc::new(StubAuditLogService));
        let vault_id = Uuid::new_v4();
        let other_vault_id = Uuid::new_v4();
        let vault_key = SecretBox::new(Box::new(vec![8_u8; 32]));

        let first_result = service
            .create_secret(
                vault_id,
                SecretType::Password,
                Some("Premier".to_string()),
                None,
                None,
                None,
                SecretBox::new(Box::new(b"first-secret".to_vec())),
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            )
            .await;
        let second_result = service
            .create_secret(
                vault_id,
                SecretType::ApiToken,
                Some("Second".to_string()),
                None,
                None,
                None,
                SecretBox::new(Box::new(b"second-secret".to_vec())),
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            )
            .await;
        let third_result = service
            .create_secret(
                other_vault_id,
                SecretType::SshKey,
                Some("Troisieme".to_string()),
                None,
                None,
                None,
                SecretBox::new(Box::new(b"third-secret".to_vec())),
                vault_key,
            )
            .await;

        assert!(first_result.is_ok(), "first create should succeed");
        assert!(second_result.is_ok(), "second create should succeed");
        assert!(third_result.is_ok(), "third create should succeed");
        if first_result.is_err() || second_result.is_err() || third_result.is_err() {
            return;
        }

        let first = match first_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let listed_before_result = service.list_by_vault(vault_id).await;
        assert!(
            listed_before_result.is_ok(),
            "list before delete should succeed"
        );
        let listed_before = match listed_before_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(listed_before.len(), 2);

        let delete_result = service.soft_delete(first.id).await;
        assert!(delete_result.is_ok(), "soft delete should succeed");
        if delete_result.is_err() {
            return;
        }

        let listed_after_result = service.list_by_vault(vault_id).await;
        assert!(
            listed_after_result.is_ok(),
            "list after delete should succeed"
        );
        let listed_after = match listed_after_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(listed_after.len(), 1);
        assert_ne!(listed_after[0].id, first.id);
    }

    #[tokio::test]
    async fn update_secret_without_new_payload_keeps_existing_blob() {
        let repo = StubSecretRepository::default();
        let service =
            SecretServiceImpl::new(repo, StubCryptoService, Arc::new(StubAuditLogService));
        let vault_id = Uuid::new_v4();
        let vault_key = SecretBox::new(Box::new(vec![6_u8; 32]));

        let created_result = service
            .create_secret(
                vault_id,
                SecretType::Password,
                Some("Avant".to_string()),
                Some("{\"category\":\"Ops\"}".to_string()),
                Some("init".to_string()),
                None,
                SecretBox::new(Box::new(b"unchanged-secret".to_vec())),
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            )
            .await;
        assert!(created_result.is_ok(), "create should succeed");
        let created = match created_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let update_result = service
            .update_secret(
                created.id,
                Some("Apres".to_string()),
                Some("{\"category\":\"Infra\"}".to_string()),
                Some("prod".to_string()),
                Some("2026-12-24T00:00:00Z".to_string()),
                None,
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
                None,
            )
            .await;
        assert!(update_result.is_ok(), "update should succeed");
        if update_result.is_err() {
            return;
        }

        let decrypted_result = service
            .get_secret(
                created.id,
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            )
            .await;
        assert!(decrypted_result.is_ok(), "get should succeed");
        let decrypted = match decrypted_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(
            decrypted.secret_value.expose_secret().as_slice(),
            b"unchanged-secret"
        );

        let listed_result = service.list_by_vault(vault_id).await;
        assert!(listed_result.is_ok(), "list should succeed");
        let listed = match listed_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title.as_deref(), Some("Apres"));
        assert_eq!(
            listed[0].metadata_json.as_deref(),
            Some("{\"category\":\"Infra\"}")
        );
    }

    #[tokio::test]
    async fn move_secret_transfers_item_between_vaults() {
        let repo = StubSecretRepository::default();
        let service =
            SecretServiceImpl::new(repo, StubCryptoService, Arc::new(StubAuditLogService));
        let source_vault_id = Uuid::new_v4();
        let target_vault_id = Uuid::new_v4();
        let source_key = SecretBox::new(Box::new(vec![1_u8; 32]));
        let target_key = SecretBox::new(Box::new(vec![2_u8; 32]));

        let created_result = service
            .create_secret(
                source_vault_id,
                SecretType::Password,
                Some("Move me".to_string()),
                None,
                None,
                None,
                SecretBox::new(Box::new(b"secret-before-move".to_vec())),
                SecretBox::new(Box::new(source_key.expose_secret().clone())),
            )
            .await;
        assert!(created_result.is_ok(), "create should succeed");
        let created = match created_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let move_result = service
            .move_secret(
                created.id,
                target_vault_id,
                SecretBox::new(Box::new(source_key.expose_secret().clone())),
                SecretBox::new(Box::new(target_key.expose_secret().clone())),
            )
            .await;
        assert!(move_result.is_ok(), "move should succeed");
        if move_result.is_err() {
            return;
        }

        let source_items_result = service.list_by_vault(source_vault_id).await;
        assert!(source_items_result.is_ok(), "source list should succeed");
        let source_items = match source_items_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(source_items.is_empty());

        let target_items_result = service.list_by_vault(target_vault_id).await;
        assert!(target_items_result.is_ok(), "target list should succeed");
        let target_items = match target_items_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(target_items.len(), 1);
        assert_eq!(target_items[0].id, created.id);

        let decrypted_result = service
            .get_secret(
                created.id,
                SecretBox::new(Box::new(target_key.expose_secret().clone())),
            )
            .await;
        assert!(decrypted_result.is_ok(), "get after move should succeed");
        let decrypted = match decrypted_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(decrypted.vault_id, target_vault_id);
        assert_eq!(
            decrypted.secret_value.expose_secret().as_slice(),
            b"secret-before-move"
        );
    }

    #[tokio::test]
    async fn trash_lifecycle_and_empty_trash_are_scoped() {
        let repo = StubSecretRepository::default();
        let service =
            SecretServiceImpl::new(repo, StubCryptoService, Arc::new(StubAuditLogService));
        let vault_a = Uuid::new_v4();
        let vault_b = Uuid::new_v4();
        let vault_key = SecretBox::new(Box::new(vec![5_u8; 32]));

        let item_a_result = service
            .create_secret(
                vault_a,
                SecretType::Password,
                Some("A".to_string()),
                None,
                None,
                None,
                SecretBox::new(Box::new(b"secret-a".to_vec())),
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            )
            .await;
        let item_b_result = service
            .create_secret(
                vault_b,
                SecretType::Password,
                Some("B".to_string()),
                None,
                None,
                None,
                SecretBox::new(Box::new(b"secret-b".to_vec())),
                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            )
            .await;
        assert!(
            item_a_result.is_ok() && item_b_result.is_ok(),
            "create should succeed"
        );
        let item_a = match item_a_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let item_b = match item_b_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let del_a = service.soft_delete(item_a.id).await;
        let del_b = service.soft_delete(item_b.id).await;
        assert!(del_a.is_ok() && del_b.is_ok(), "soft delete should succeed");
        if del_a.is_err() || del_b.is_err() {
            return;
        }

        let trash_a_result = service.list_trash_by_vault(vault_a).await;
        assert!(trash_a_result.is_ok(), "trash list should succeed");
        let trash_a = match trash_a_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(trash_a.len(), 1);
        assert_eq!(trash_a[0].id, item_a.id);

        let restore_result = service.restore_secret(item_a.id, vault_a).await;
        assert!(restore_result.is_ok(), "restore should succeed");
        if restore_result.is_err() {
            return;
        }

        let active_a_result = service.list_by_vault(vault_a).await;
        assert!(active_a_result.is_ok(), "active list should succeed");
        let active_a = match active_a_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(active_a.len(), 1);

        let del_a_again = service.soft_delete(item_a.id).await;
        assert!(del_a_again.is_ok(), "soft delete should succeed again");
        if del_a_again.is_err() {
            return;
        }

        let empty_a_result = service.empty_trash(vault_a).await;
        assert!(empty_a_result.is_ok(), "empty trash should succeed");
        let empty_a = match empty_a_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(empty_a, 1);

        let trash_b_result = service.list_trash_by_vault(vault_b).await;
        assert!(trash_b_result.is_ok(), "vault b trash should remain");
        let trash_b = match trash_b_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(trash_b.len(), 1);

        let hard_delete_result = service.permanent_delete(item_b.id, vault_b).await;
        assert!(
            hard_delete_result.is_ok(),
            "permanent delete should succeed"
        );
        if hard_delete_result.is_err() {
            return;
        }

        let trash_b_after_result = service.list_trash_by_vault(vault_b).await;
        assert!(
            trash_b_after_result.is_ok(),
            "vault b trash should be empty"
        );
        let trash_b_after = match trash_b_after_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(trash_b_after.is_empty());
    }
}

use heelonvault_core::services::secret_service::DecryptedSecret;

use super::*;

pub(super) struct ControlledSecret<T> {
    pub(super) inner: Arc<T>,
    pub(super) fail_list: Mutex<Option<Uuid>>,
}

impl<T: SecretService + Send + Sync> SecretService for ControlledSecret<T> {
    async fn list_by_vault(&self, id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        let failed = *self
            .fail_list
            .lock()
            .map_err(|_| AppError::Storage("test fault poisoned".to_string()))?
            == Some(id);
        if failed {
            return Err(AppError::Storage(
                "synthetic vault listing failure".to_string(),
            ));
        }
        self.inner.list_by_vault(id).await
    }
    async fn create_secret(
        &self,
        vault: Uuid,
        kind: SecretType,
        title: Option<String>,
        metadata: Option<String>,
        tags: Option<String>,
        expires: Option<String>,
        value: SecretBox<Vec<u8>>,
        key: SecretBox<Vec<u8>>,
    ) -> Result<SecretItem, AppError> {
        self.inner
            .create_secret(vault, kind, title, metadata, tags, expires, value, key)
            .await
    }
    async fn get_secret(
        &self,
        id: Uuid,
        key: SecretBox<Vec<u8>>,
    ) -> Result<DecryptedSecret, AppError> {
        self.inner.get_secret(id, key).await
    }
    async fn update_secret(
        &self,
        id: Uuid,
        title: Option<String>,
        metadata: Option<String>,
        tags: Option<String>,
        expires: Option<String>,
        value: Option<SecretBox<Vec<u8>>>,
        key: SecretBox<Vec<u8>>,
        audit: Option<String>,
    ) -> Result<(), AppError> {
        self.inner
            .update_secret(id, title, metadata, tags, expires, value, key, audit)
            .await
    }
    async fn record_viewed(
        &self,
        id: Uuid,
        actor: Option<Uuid>,
        title: Option<&str>,
    ) -> Result<(), AppError> {
        self.inner.record_viewed(id, actor, title).await
    }
    async fn record_field_copy(
        &self,
        id: Uuid,
        actor: Option<Uuid>,
        title: Option<&str>,
        field: &str,
    ) -> Result<(), AppError> {
        self.inner.record_field_copy(id, actor, title, field).await
    }
    async fn move_secret(
        &self,
        id: Uuid,
        target: Uuid,
        source_key: SecretBox<Vec<u8>>,
        target_key: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        self.inner
            .move_secret(id, target, source_key, target_key)
            .await
    }
    async fn list_trash_by_vault(&self, id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        self.inner.list_trash_by_vault(id).await
    }
    async fn list_all_trash_by_user(&self, id: Uuid) -> Result<Vec<SecretItem>, AppError> {
        self.inner.list_all_trash_by_user(id).await
    }
    async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        self.inner.soft_delete(id).await
    }
    async fn restore_secret(&self, id: Uuid, vault: Uuid) -> Result<(), AppError> {
        self.inner.restore_secret(id, vault).await
    }
    async fn permanent_delete(&self, id: Uuid, vault: Uuid) -> Result<(), AppError> {
        self.inner.permanent_delete(id, vault).await
    }
    async fn empty_trash(&self, id: Uuid) -> Result<usize, AppError> {
        self.inner.empty_trash(id).await
    }
    async fn increment_usage_count(&self, id: Uuid) -> Result<(), AppError> {
        self.inner.increment_usage_count(id).await
    }
}

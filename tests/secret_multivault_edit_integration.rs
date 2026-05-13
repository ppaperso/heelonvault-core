#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use heelonvault_rust::errors::AppError;
use heelonvault_rust::models::{SecretType, UserRole};
use heelonvault_rust::repositories::audit_log_repository::SqlxAuditLogRepository;
use heelonvault_rust::repositories::secret_repository::SqlxSecretRepository;
use heelonvault_rust::repositories::team_repository::SqlxTeamRepository;
use heelonvault_rust::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_rust::repositories::vault_repository::SqlxVaultRepository;
use heelonvault_rust::services::audit_log_service::AuditLogServiceImpl;
use heelonvault_rust::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_rust::services::crypto_service::CryptoServiceImpl;
use heelonvault_rust::services::secret_service::{SecretService, SecretServiceImpl};
use heelonvault_rust::services::vault_service::{
    VaultKeyEnvelopeRepository, VaultService, VaultServiceImpl,
};
use secrecy::{ExposeSecret, SecretBox};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

struct SqlxVaultEnvelopeRepository {
    pool: SqlitePool,
}

impl SqlxVaultEnvelopeRepository {
    fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl VaultKeyEnvelopeRepository for SqlxVaultEnvelopeRepository {
    async fn get_vault_key_envelope(
        &self,
        vault_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        let row_opt = sqlx::query("SELECT vault_key_envelope FROM vaults WHERE id = ?1")
            .bind(vault_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)?;

        let envelope = row_opt
            .and_then(|row| row.try_get::<Option<Vec<u8>>, _>("vault_key_envelope").ok())
            .flatten();

        Ok(envelope.map(|bytes| SecretBox::new(Box::new(bytes))))
    }
}

type AuditSvc = AuditLogServiceImpl<SqlxUserRepository, SqlxAuditLogRepository>;
type VaultSvc = VaultServiceImpl<
    SqlxVaultRepository,
    SqlxVaultEnvelopeRepository,
    SqlxUserRepository,
    SqlxTeamRepository,
    AuditSvc,
    CryptoServiceImpl,
>;
type SecretSvc = SecretServiceImpl<SqlxSecretRepository, CryptoServiceImpl, AuditSvc>;

struct UserSeed {
    id: Uuid,
    master_key: SecretBox<Vec<u8>>,
}

struct TestCtx {
    admin: UserSeed,
    secret_service: Arc<SecretSvc>,
    vault_service: Arc<VaultSvc>,
}

async fn create_account(
    user_repo: &SqlxUserRepository,
    auth: Arc<AuthServiceImpl<CryptoServiceImpl>>,
    username: &str,
    password: &str,
    role: UserRole,
) -> Result<UserSeed, String> {
    let password_bytes = password.as_bytes().to_vec();
    let password_secret = SecretBox::new(Box::new(password_bytes.clone()));

    auth.create_user(username, password_secret)
        .await
        .map_err(|err| format!("auth create_user {username}: {err}"))?;

    let master_key = auth
        .derive_key_if_valid(username, SecretBox::new(Box::new(password_bytes)))
        .await
        .map_err(|err| format!("derive key {username}: {err}"))?
        .ok_or_else(|| format!("missing derived key for {username}"))?;

    let user_id = Uuid::new_v4();
    user_repo
        .create_user_db(user_id, username, &role)
        .await
        .map_err(|err| format!("create user row {username}: {err}"))?;

    let envelope = auth
        .get_password_envelope(username)
        .await
        .map_err(|err| format!("get envelope {username}: {err}"))?;

    user_repo
        .update_password_envelope(user_id, envelope)
        .await
        .map_err(|err| format!("persist envelope {username}: {err}"))?;

    Ok(UserSeed {
        id: user_id,
        master_key,
    })
}

async fn setup_ctx() -> Result<TestCtx, String> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .map_err(|err| format!("connect sqlite memory: {err}"))?;

    sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await
        .map_err(|err| format!("load migrations: {err}"))?
        .run(&pool)
        .await
        .map_err(|err| format!("run migrations: {err}"))?;

    let auth = Arc::new(AuthServiceImpl::new(CryptoServiceImpl::with_defaults()));
    let user_repo = SqlxUserRepository::new(pool.clone());
    let audit_service = Arc::new(AuditLogServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        SqlxAuditLogRepository::new(pool.clone()),
    ));

    let admin = create_account(
        &user_repo,
        Arc::clone(&auth),
        "admin_edit",
        "Admin1234!",
        UserRole::Admin,
    )
    .await?;

    let vault_service = Arc::new(VaultServiceImpl::new(
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultEnvelopeRepository::new(pool.clone()),
        SqlxUserRepository::new(pool.clone()),
        SqlxTeamRepository::new(pool.clone()),
        Arc::clone(&audit_service),
        CryptoServiceImpl::with_defaults(),
    ));

    let secret_service = Arc::new(SecretServiceImpl::new(
        SqlxSecretRepository::new(pool),
        CryptoServiceImpl::with_defaults(),
        audit_service,
    ));

    Ok(TestCtx {
        admin,
        secret_service,
        vault_service,
    })
}

async fn resolve_secret_vault_for_user(
    vault_service: &VaultSvc,
    secret_service: &SecretSvc,
    user_id: Uuid,
    secret_id: Uuid,
) -> Result<Uuid, AppError> {
    let vaults = vault_service.list_user_vaults(user_id).await?;
    for vault in vaults {
        let items = secret_service.list_by_vault(vault.id).await?;
        if items.into_iter().any(|item| item.id == secret_id) {
            return Ok(vault.id);
        }
    }

    Err(AppError::NotFound("secret not found".to_string()))
}

#[tokio::test]
async fn scenario_edit_secret_locates_non_first_vault() {
    let ctx_result = setup_ctx().await;
    assert!(ctx_result.is_ok(), "setup must succeed");
    let ctx = match ctx_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let vault_a = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "A Coffre",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create first vault");

    let vault_b = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "B Coffre",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create second vault");

    let vault_b_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault_b.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("open second vault");

    let secret = ctx
        .secret_service
        .create_secret(
            vault_b.id,
            SecretType::Password,
            Some("prod-login".to_string()),
            Some("{\"login\":\"isa\"}".to_string()),
            Some("prod".to_string()),
            None,
            SecretBox::new(Box::new(b"old-pass".to_vec())),
            SecretBox::new(Box::new(vault_b_key.expose_secret().clone())),
        )
        .await
        .expect("create secret in second vault");

    let first_vault_items = ctx
        .secret_service
        .list_by_vault(vault_a.id)
        .await
        .expect("list first vault secrets");
    assert!(
        first_vault_items
            .into_iter()
            .all(|item| item.id != secret.id),
        "first vault should not contain the target secret"
    );

    let resolved_vault_id = resolve_secret_vault_for_user(
        ctx.vault_service.as_ref(),
        ctx.secret_service.as_ref(),
        ctx.admin.id,
        secret.id,
    )
    .await
    .expect("resolve vault containing target secret");
    assert_eq!(
        resolved_vault_id, vault_b.id,
        "secret must resolve to the second vault"
    );

    let resolved_vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            resolved_vault_id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("open resolved vault");

    let update_result = ctx
        .secret_service
        .update_secret(
            secret.id,
            Some("prod-login-updated".to_string()),
            Some("{\"login\":\"isa\",\"notes\":\"rotated\"}".to_string()),
            Some("prod,rotated".to_string()),
            None,
            Some(SecretBox::new(Box::new(b"new-pass".to_vec()))),
            SecretBox::new(Box::new(resolved_vault_key.expose_secret().clone())),
        )
        .await;
    assert!(
        update_result.is_ok(),
        "update must succeed for non-first vault secret"
    );

    let decrypted_after = ctx
        .secret_service
        .get_secret(
            secret.id,
            SecretBox::new(Box::new(resolved_vault_key.expose_secret().clone())),
        )
        .await
        .expect("decrypt updated secret");
    assert_eq!(
        decrypted_after.secret_value.expose_secret().as_slice(),
        b"new-pass",
        "updated secret payload must be readable with the resolved vault key"
    );

    let second_vault_items = ctx
        .secret_service
        .list_by_vault(vault_b.id)
        .await
        .expect("list second vault secrets");
    let updated = second_vault_items
        .into_iter()
        .find(|item| item.id == secret.id)
        .expect("updated secret must still be in second vault");
    assert_eq!(updated.title.as_deref(), Some("prod-login-updated"));
}

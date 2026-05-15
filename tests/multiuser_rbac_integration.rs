#![allow(clippy::disallowed_methods, clippy::redundant_closure)]

use std::sync::Arc;

use heelonvault_rust::errors::{AccessDeniedReason, AppError};
use heelonvault_rust::models::{SecretType, UserRole, VaultShareRole};
use heelonvault_rust::repositories::audit_log_repository::SqlxAuditLogRepository;
use heelonvault_rust::repositories::secret_repository::SqlxSecretRepository;
use heelonvault_rust::repositories::team_repository::SqlxTeamRepository;
use heelonvault_rust::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_rust::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_rust::services::admin_service::{AdminService, AdminServiceImpl};
use heelonvault_rust::services::audit_log_service::AuditLogServiceImpl;
use heelonvault_rust::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_rust::services::crypto_service::{
    CryptoService, CryptoServiceImpl, EncryptedPayload, NONCE_LEN,
};
use heelonvault_rust::services::secret_service::{SecretService, SecretServiceImpl};
use heelonvault_rust::services::team_service::{KeyShare, TeamService, TeamServiceImpl};
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
type AdminSvc = AdminServiceImpl<SqlxUserRepository, AuthServiceImpl<CryptoServiceImpl>, AuditSvc>;
type TeamSvc = TeamServiceImpl<
    SqlxTeamRepository,
    SqlxUserRepository,
    SqlxVaultRepository,
    CryptoServiceImpl,
    AuditSvc,
>;
type SecretSvc = SecretServiceImpl<SqlxSecretRepository, CryptoServiceImpl, AuditSvc>;
type VaultSvc = VaultServiceImpl<
    SqlxVaultRepository,
    SqlxVaultEnvelopeRepository,
    SqlxUserRepository,
    SqlxTeamRepository,
    AuditSvc,
    CryptoServiceImpl,
>;

struct UserSeed {
    id: Uuid,
    master_key: SecretBox<Vec<u8>>,
}

struct TestCtx {
    pool: SqlitePool,
    auth: Arc<AuthServiceImpl<CryptoServiceImpl>>,
    admin: UserSeed,
    admin_service: Arc<AdminSvc>,
    team_service: Arc<TeamSvc>,
    secret_service: Arc<SecretSvc>,
    vault_service: Arc<VaultSvc>,
    crypto: CryptoServiceImpl,
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
        "admin",
        "Admin1234!",
        UserRole::Admin,
    )
    .await?;

    let admin_service = Arc::new(AdminServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        Arc::clone(&auth),
        Arc::clone(&audit_service),
    ));

    let team_service = Arc::new(TeamServiceImpl::new(
        SqlxTeamRepository::new(pool.clone()),
        SqlxUserRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        CryptoServiceImpl::with_defaults(),
        Arc::clone(&audit_service),
    ));

    let secret_service = Arc::new(SecretServiceImpl::new(
        SqlxSecretRepository::new(pool.clone()),
        CryptoServiceImpl::with_defaults(),
        Arc::clone(&audit_service),
    ));

    let vault_service = Arc::new(VaultServiceImpl::new(
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultEnvelopeRepository::new(pool.clone()),
        SqlxUserRepository::new(pool.clone()),
        SqlxTeamRepository::new(pool.clone()),
        Arc::clone(&audit_service),
        CryptoServiceImpl::with_defaults(),
    ));

    Ok(TestCtx {
        pool,
        auth,
        admin,
        admin_service,
        team_service,
        secret_service,
        vault_service,
        crypto: CryptoServiceImpl::with_defaults(),
    })
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

fn serialize_payload(payload: &EncryptedPayload) -> SecretBox<Vec<u8>> {
    let mut bytes = Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
    bytes.extend_from_slice(&payload.nonce);
    bytes.extend_from_slice(payload.ciphertext.expose_secret());
    SecretBox::new(Box::new(bytes))
}

async fn create_secret_via_ui_service_flow(
    secret_service: &SecretSvc,
    vault_service: &VaultSvc,
    requester_id: Uuid,
    target_vault_id: Uuid,
    requester_master_key: SecretBox<Vec<u8>>,
    title: &str,
    secret_value: &[u8],
) -> Result<(), AppError> {
    let access = vault_service
        .get_vault_access_for_user(requester_id, target_vault_id)
        .await?
        .ok_or(AppError::Authorization(
            AccessDeniedReason::VaultAccessDenied,
        ))?;

    let is_shared = access.vault.owner_user_id != requester_id;
    if is_shared && !access.role.can_admin() {
        return Err(AppError::Authorization(
            AccessDeniedReason::VaultSharedCreateDenied,
        ));
    }

    let vault_key = vault_service
        .open_vault_for_user(requester_id, target_vault_id, requester_master_key)
        .await?;

    secret_service
        .create_secret(
            target_vault_id,
            SecretType::Password,
            Some(title.to_string()),
            Some("{\"login\":\"shared-user\"}".to_string()),
            None,
            None,
            SecretBox::new(Box::new(secret_value.to_vec())),
            SecretBox::new(Box::new(vault_key.expose_secret().clone())),
        )
        .await
        .map(|_| ())
}

#[tokio::test]
async fn scenario_member_removal_purges_shares_and_blocks_open() {
    let ctx_result = setup_ctx().await;
    assert!(ctx_result.is_ok(), "setup must succeed");
    let ctx = match ctx_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let member = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "member_a",
        "Member1234!",
        UserRole::User,
    )
    .await
    .expect("create member account");

    let team = ctx
        .team_service
        .create_team(ctx.admin.id, "Blue Team")
        .await
        .expect("create team");
    ctx.team_service
        .add_member(
            ctx.admin.id,
            team.id,
            member.id,
            heelonvault_rust::models::TeamMemberRole::Member,
        )
        .await
        .expect("add member");

    let vault = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "Clinical Vault",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create vault");

    let vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("open admin vault");

    ctx.team_service
        .share_vault_with_team(
            ctx.admin.id,
            vault.id,
            team.id,
            SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            &[(
                member.id,
                SecretBox::new(Box::new(member.master_key.expose_secret().clone())),
            )],
        )
        .await
        .expect("share with team");

    let member_open_before = ctx
        .vault_service
        .open_vault_for_user(
            member.id,
            vault.id,
            SecretBox::new(Box::new(member.master_key.expose_secret().clone())),
        )
        .await;
    assert!(
        member_open_before.is_ok(),
        "member should open before removal"
    );

    ctx.team_service
        .remove_member(ctx.admin.id, team.id, member.id)
        .await
        .expect("remove member");

    let count_row = sqlx::query(
        "SELECT COUNT(*) AS count FROM vault_key_shares WHERE user_id = ?1 AND granted_via_team = ?2",
    )
    .bind(member.id.to_string())
    .bind(team.id.to_string())
    .fetch_one(&ctx.pool)
    .await
    .expect("count rows");
    let share_count: i64 = count_row.try_get("count").expect("count value");
    assert_eq!(share_count, 0, "team-based shares must be purged");

    let member_open_after = ctx
        .vault_service
        .open_vault_for_user(
            member.id,
            vault.id,
            SecretBox::new(Box::new(member.master_key.expose_secret().clone())),
        )
        .await;
    assert!(
        matches!(member_open_after, Err(AppError::Authorization(_))),
        "removed member must not open vault"
    );
}

#[tokio::test]
async fn scenario_key_rotation_keeps_remaining_members_and_blocks_excluded() {
    let ctx_result = setup_ctx().await;
    assert!(ctx_result.is_ok(), "setup must succeed");
    let ctx = match ctx_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let member_a = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "member_rot_a",
        "MemberA1234!",
        UserRole::User,
    )
    .await
    .expect("create member a");
    let member_b = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "member_rot_b",
        "MemberB1234!",
        UserRole::User,
    )
    .await
    .expect("create member b");
    let excluded = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "member_rot_ex",
        "MemberEx1234!",
        UserRole::User,
    )
    .await
    .expect("create excluded member");

    let team = ctx
        .team_service
        .create_team(ctx.admin.id, "Rotation Team")
        .await
        .expect("create team");
    ctx.team_service
        .add_member(
            ctx.admin.id,
            team.id,
            member_a.id,
            heelonvault_rust::models::TeamMemberRole::Member,
        )
        .await
        .expect("add a");
    ctx.team_service
        .add_member(
            ctx.admin.id,
            team.id,
            member_b.id,
            heelonvault_rust::models::TeamMemberRole::Member,
        )
        .await
        .expect("add b");
    ctx.team_service
        .add_member(
            ctx.admin.id,
            team.id,
            excluded.id,
            heelonvault_rust::models::TeamMemberRole::Member,
        )
        .await
        .expect("add excluded");

    let vault = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "Rotation Vault",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create vault");

    let current_vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("open vault as admin");

    ctx.team_service
        .share_vault_with_team(
            ctx.admin.id,
            vault.id,
            team.id,
            SecretBox::new(Box::new(current_vault_key.expose_secret().clone())),
            &[
                (
                    member_a.id,
                    SecretBox::new(Box::new(member_a.master_key.expose_secret().clone())),
                ),
                (
                    member_b.id,
                    SecretBox::new(Box::new(member_b.master_key.expose_secret().clone())),
                ),
                (
                    excluded.id,
                    SecretBox::new(Box::new(excluded.master_key.expose_secret().clone())),
                ),
            ],
        )
        .await
        .expect("share before rotation");

    ctx.team_service
        .remove_member(ctx.admin.id, team.id, excluded.id)
        .await
        .expect("remove excluded");

    let new_vault_key = SecretBox::new(Box::new(vec![0x33_u8; 32]));
    let owner_payload = ctx
        .crypto
        .encrypt(
            &new_vault_key,
            &SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("encrypt owner envelope");
    let new_owner_envelope = serialize_payload(&owner_payload);

    let mut new_shares: Vec<KeyShare> = Vec::new();
    for (user_id, user_master) in [
        (
            member_a.id,
            SecretBox::new(Box::new(member_a.master_key.expose_secret().clone())),
        ),
        (
            member_b.id,
            SecretBox::new(Box::new(member_b.master_key.expose_secret().clone())),
        ),
    ] {
        let payload = ctx
            .crypto
            .encrypt(&new_vault_key, &user_master)
            .await
            .expect("encrypt member envelope");
        new_shares.push((user_id, serialize_payload(&payload), Some(team.id)));
    }

    let rotate_result = ctx
        .team_service
        .rotate_vault_key(ctx.admin.id, vault.id, new_owner_envelope, new_shares)
        .await;
    assert!(rotate_result.is_ok(), "rotation must succeed");

    let owner_opened = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("owner opens after rotation");
    assert_eq!(owner_opened.expose_secret(), new_vault_key.expose_secret());

    let a_opened = ctx
        .vault_service
        .open_vault_for_user(
            member_a.id,
            vault.id,
            SecretBox::new(Box::new(member_a.master_key.expose_secret().clone())),
        )
        .await;
    assert!(a_opened.is_ok(), "member A must still open after rotation");

    let b_opened = ctx
        .vault_service
        .open_vault_for_user(
            member_b.id,
            vault.id,
            SecretBox::new(Box::new(member_b.master_key.expose_secret().clone())),
        )
        .await;
    assert!(b_opened.is_ok(), "member B must still open after rotation");

    let ex_opened = ctx
        .vault_service
        .open_vault_for_user(
            excluded.id,
            vault.id,
            SecretBox::new(Box::new(excluded.master_key.expose_secret().clone())),
        )
        .await;
    assert!(matches!(ex_opened, Err(AppError::Authorization(_))));
}

#[tokio::test]
async fn scenario_audit_trail_admin_actions_are_complete() {
    let ctx_result = setup_ctx().await;
    assert!(ctx_result.is_ok(), "setup must succeed");
    let ctx = match ctx_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let create_result = ctx
        .admin_service
        .create_user(
            ctx.admin.id,
            "audit_user",
            SecretBox::new(Box::new(b"Audit1234!".to_vec())),
            UserRole::User,
        )
        .await;
    assert!(create_result.is_ok(), "admin create user must succeed");
    let created = match create_result {
        Ok(value) => value.user,
        Err(_) => return,
    };

    let role_result = ctx
        .admin_service
        .update_user_role(ctx.admin.id, created.id, UserRole::Admin)
        .await;
    assert!(role_result.is_ok(), "admin role update must succeed");

    let reset_result = ctx
        .admin_service
        .reset_user_password(
            ctx.admin.id,
            created.id,
            SecretBox::new(Box::new(b"Reset1234!".to_vec())),
        )
        .await;
    assert!(reset_result.is_ok(), "admin password reset must succeed");

    let delete_result = ctx
        .admin_service
        .delete_user(ctx.admin.id, created.id)
        .await;
    assert!(delete_result.is_ok(), "admin delete user must succeed");

    let rows = sqlx::query(
        "SELECT action, actor_user_id, target_type, target_id
         FROM audit_log
         WHERE target_type = 'user' AND target_id = ?1
         ORDER BY id ASC",
    )
    .bind(created.id.to_string())
    .fetch_all(&ctx.pool)
    .await
    .expect("query audit rows");

    let actions: Vec<String> = rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("action").ok())
        .collect();

    assert!(actions.contains(&"user.created".to_string()));
    assert!(actions.contains(&"user.role_changed".to_string()));
    assert!(actions.contains(&"user.password_reset".to_string()));
    assert!(actions.contains(&"user.deleted".to_string()));

    for row in &rows {
        let actor_opt: Option<String> = row.try_get("actor_user_id").expect("actor field");
        assert_eq!(
            actor_opt.as_deref(),
            Some(ctx.admin.id.to_string().as_str())
        );
        let target_type: String = row.try_get("target_type").expect("target_type");
        assert_eq!(target_type, "user");
    }
}

// ── RBAC role-gating tests ────────────────────────────────────────────────────

/// A user with `read` role can open the vault but receives `Read` access kind.
#[tokio::test]
async fn scenario_read_role_user_can_open_but_access_kind_is_read() {
    let ctx = setup_ctx().await.expect("setup");

    let reader = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "reader_rbac",
        "Read1234!",
        UserRole::User,
    )
    .await
    .expect("create reader");

    let vault = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "Read-Only Vault",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create vault");

    // Derive the vault key by opening as admin.
    let vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("admin opens vault");

    // Encrypt vault key with reader's master key and insert a Read-role share.
    let crypto = CryptoServiceImpl::with_defaults();
    let payload = crypto
        .encrypt(&vault_key, &reader.master_key)
        .await
        .expect("encrypt for reader");
    let mut envelope_bytes =
        Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
    envelope_bytes.extend_from_slice(&payload.nonce);
    envelope_bytes.extend_from_slice(payload.ciphertext.expose_secret());
    let envelope = SecretBox::new(Box::new(envelope_bytes));

    SqlxVaultRepository::new(ctx.pool.clone())
        .insert_key_share(
            vault.id,
            reader.id,
            envelope,
            Some(ctx.admin.id),
            None,
            VaultShareRole::Read,
        )
        .await
        .expect("insert read share");

    // Reader can open the vault.
    let open_result = ctx
        .vault_service
        .open_vault_for_user(
            reader.id,
            vault.id,
            SecretBox::new(Box::new(reader.master_key.expose_secret().clone())),
        )
        .await;
    assert!(
        open_result.is_ok(),
        "read-role user must be able to open vault"
    );

    // get_vault_access_for_user returns Read role.
    let access = ctx
        .vault_service
        .get_vault_access_for_user(reader.id, vault.id)
        .await
        .expect("get access")
        .expect("access record present");

    assert!(
        !access.role.can_write(),
        "read role must not have write permission"
    );
    assert!(
        !access.role.can_admin(),
        "read role must not have admin permission"
    );
}

/// A user with `write` role can open the vault and has write permission but cannot delete it.
#[tokio::test]
async fn scenario_write_role_can_open_but_not_delete() {
    let ctx = setup_ctx().await.expect("setup");

    let writer = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "writer_rbac",
        "Write1234!",
        UserRole::User,
    )
    .await
    .expect("create writer");

    let vault = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "Write Vault",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create vault");

    // Use team_service.grant_vault_access which inserts Write role.
    let vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("admin opens vault");

    ctx.team_service
        .grant_vault_access(
            ctx.admin.id,
            vault.id,
            writer.id,
            SecretBox::new(Box::new(vault_key.expose_secret().clone())),
            SecretBox::new(Box::new(writer.master_key.expose_secret().clone())),
        )
        .await
        .expect("grant write access");

    // Writer can open.
    let open_result = ctx
        .vault_service
        .open_vault_for_user(
            writer.id,
            vault.id,
            SecretBox::new(Box::new(writer.master_key.expose_secret().clone())),
        )
        .await;
    assert!(open_result.is_ok(), "write-role user must open vault");

    // Confirmed write permission via get_vault_access_for_user.
    let access = ctx
        .vault_service
        .get_vault_access_for_user(writer.id, vault.id)
        .await
        .expect("get access")
        .expect("access record");

    assert!(
        access.role.can_write(),
        "write role must have write permission"
    );

    // Writer cannot delete vault (requires admin or owner).
    let delete_result = ctx.vault_service.delete_vault(writer.id, vault.id).await;
    assert!(
        matches!(delete_result, Err(AppError::Authorization(_))),
        "write-role user must not delete vault"
    );
}

/// A user with `admin` role share can delete the vault.
#[tokio::test]
async fn scenario_admin_role_shared_user_can_delete_vault() {
    let ctx = setup_ctx().await.expect("setup");

    let vault_admin = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "vaultadmin_rbac",
        "VAdmin1234!",
        UserRole::User,
    )
    .await
    .expect("create vault_admin user");

    let vault = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "Admin-Shared Vault",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create vault");

    // Insert an Admin-role share directly via repository.
    let vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("admin opens vault");

    let crypto = CryptoServiceImpl::with_defaults();
    let payload = crypto
        .encrypt(&vault_key, &vault_admin.master_key)
        .await
        .expect("encrypt for vault_admin");
    let mut envelope_bytes =
        Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
    envelope_bytes.extend_from_slice(&payload.nonce);
    envelope_bytes.extend_from_slice(payload.ciphertext.expose_secret());
    let envelope = SecretBox::new(Box::new(envelope_bytes));

    SqlxVaultRepository::new(ctx.pool.clone())
        .insert_key_share(
            vault.id,
            vault_admin.id,
            envelope,
            Some(ctx.admin.id),
            None,
            VaultShareRole::Admin,
        )
        .await
        .expect("insert admin share");

    // The vault-admin can open the vault.
    let open_result = ctx
        .vault_service
        .open_vault_for_user(
            vault_admin.id,
            vault.id,
            SecretBox::new(Box::new(vault_admin.master_key.expose_secret().clone())),
        )
        .await;
    assert!(open_result.is_ok(), "admin-role user must open vault");

    // The vault-admin can delete the vault.
    let delete_result = ctx
        .vault_service
        .delete_vault(vault_admin.id, vault.id)
        .await;
    assert!(
        delete_result.is_ok(),
        "admin-role shared user must be able to delete vault: {:?}",
        delete_result.err()
    );

    // Vault is soft-deleted: original owner can no longer open it.
    let owner_open = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await;
    assert!(
        matches!(owner_open, Err(AppError::Authorization(_))),
        "soft-deleted vault must not be openable"
    );
}

#[tokio::test]
async fn scenario_shared_write_user_cannot_add_secret_via_ui_service_flow() {
    let ctx = setup_ctx().await.expect("setup");

    let writer = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "writer_add_guard",
        "Write1234!",
        UserRole::User,
    )
    .await
    .expect("create writer");

    let vault = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "Guarded Shared Vault",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create vault");

    let owner_vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("open owner vault");

    // grant_vault_access gives WRITE role to the user.
    ctx.team_service
        .grant_vault_access(
            ctx.admin.id,
            vault.id,
            writer.id,
            SecretBox::new(Box::new(owner_vault_key.expose_secret().clone())),
            SecretBox::new(Box::new(writer.master_key.expose_secret().clone())),
        )
        .await
        .expect("grant write access");

    let create_result = create_secret_via_ui_service_flow(
        ctx.secret_service.as_ref(),
        ctx.vault_service.as_ref(),
        writer.id,
        vault.id,
        SecretBox::new(Box::new(writer.master_key.expose_secret().clone())),
        "must-be-denied",
        b"denied-secret",
    )
    .await;

    assert!(
        matches!(create_result, Err(AppError::Authorization(_))),
        "shared user with WRITE role must be denied secret creation"
    );

    let items = ctx
        .secret_service
        .list_by_vault(vault.id)
        .await
        .expect("list vault secrets");
    assert!(
        items.is_empty(),
        "no secret must be persisted when creation is denied"
    );
}

#[tokio::test]
async fn scenario_shared_admin_user_can_add_secret_via_ui_service_flow() {
    let ctx = setup_ctx().await.expect("setup");

    let shared_admin = create_account(
        &SqlxUserRepository::new(ctx.pool.clone()),
        Arc::clone(&ctx.auth),
        "shared_admin_add",
        "Admin1234!",
        UserRole::User,
    )
    .await
    .expect("create shared admin");

    let vault = ctx
        .vault_service
        .create_vault(
            ctx.admin.id,
            "Admin Shared Vault",
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("create vault");

    let vault_key = ctx
        .vault_service
        .open_vault_for_user(
            ctx.admin.id,
            vault.id,
            SecretBox::new(Box::new(ctx.admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("open owner vault");

    let payload = ctx
        .crypto
        .encrypt(
            &vault_key,
            &SecretBox::new(Box::new(shared_admin.master_key.expose_secret().clone())),
        )
        .await
        .expect("encrypt admin share envelope");
    let envelope = serialize_payload(&payload);

    SqlxVaultRepository::new(ctx.pool.clone())
        .insert_key_share(
            vault.id,
            shared_admin.id,
            envelope,
            Some(ctx.admin.id),
            None,
            VaultShareRole::Admin,
        )
        .await
        .expect("insert admin share");

    let create_result = create_secret_via_ui_service_flow(
        ctx.secret_service.as_ref(),
        ctx.vault_service.as_ref(),
        shared_admin.id,
        vault.id,
        SecretBox::new(Box::new(shared_admin.master_key.expose_secret().clone())),
        "allowed-for-admin",
        b"created-secret",
    )
    .await;
    assert!(
        create_result.is_ok(),
        "shared user with ADMIN role must be allowed to create secret"
    );

    let items = ctx
        .secret_service
        .list_by_vault(vault.id)
        .await
        .expect("list vault secrets");
    assert_eq!(items.len(), 1, "one secret must be created");
    assert_eq!(
        items[0].title.as_deref(),
        Some("allowed-for-admin"),
        "created secret title must match"
    );
}

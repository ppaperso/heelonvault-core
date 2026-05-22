use uuid::Uuid;

/// Typed audit actions for compile-time safety.
/// Serialised as dot-namespaced strings in the DB for easy SQL filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditAction {
    // User management
    UserCreated,
    UserDeleted,
    UserRoleChanged,
    UserPasswordReset,
    // Team management
    TeamCreated,
    TeamDeleted,
    TeamMemberAdded,
    TeamMemberRemoved,
    // Vault sharing / key lifecycle
    VaultSharedWithUser,
    VaultSharedWithTeam,
    VaultAccessRevoked,
    VaultKeyRotated,
    VaultCreated,
    VaultDeleted,
    VaultOpened,
    // Secret lifecycle
    SecretCreated,
    SecretUpdated,
    SecretDeleted,
}

impl AuditAction {
    pub fn to_db_str(&self) -> &'static str {
        match self {
            AuditAction::UserCreated => "user.created",
            AuditAction::UserDeleted => "user.deleted",
            AuditAction::UserRoleChanged => "user.role_changed",
            AuditAction::UserPasswordReset => "user.password_reset",
            AuditAction::TeamCreated => "team.created",
            AuditAction::TeamDeleted => "team.deleted",
            AuditAction::TeamMemberAdded => "team.member_added",
            AuditAction::TeamMemberRemoved => "team.member_removed",
            AuditAction::VaultSharedWithUser => "vault.shared_with_user",
            AuditAction::VaultSharedWithTeam => "vault.shared_with_team",
            AuditAction::VaultAccessRevoked => "vault.access_revoked",
            AuditAction::VaultKeyRotated => "vault.key_rotated",
            AuditAction::VaultCreated => "vault.created",
            AuditAction::VaultDeleted => "vault.deleted",
            AuditAction::VaultOpened => "vault.opened",
            AuditAction::SecretCreated => "secret.created",
            AuditAction::SecretUpdated => "secret.updated",
            AuditAction::SecretDeleted => "secret.deleted",
        }
    }
}

/// A single immutable audit record returned by the repository.
#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub id: i64,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub detail: Option<String>,
    pub performed_at: String,
}

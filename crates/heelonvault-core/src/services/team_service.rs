use secrecy::SecretBox;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{Team, TeamMember, TeamMemberRole, User};

// ── public types ──────────────────────────────────────────────────────────────

/// A new encrypted key share: (recipient_user_id, vault_key encrypted with
/// recipient's master key, optional team_id context).
pub type KeyShare = (Uuid, SecretBox<Vec<u8>>, Option<Uuid>);

/// Result of `rotate_vault_key`: the new vault key in plaintext so the caller
/// can re-encrypt every secret item blob before discarding it.
pub struct KeyRotationResult {
    /// The new vault key in plaintext.  The caller MUST re-encrypt all secret
    /// blobs with this key and then zero it.
    pub new_vault_key: SecretBox<Vec<u8>>,
}

// ── trait ─────────────────────────────────────────────────────────────────────

#[trait_variant::make(TeamService: Send)]
pub trait LocalTeamService {
    async fn create_team(&self, creator_id: Uuid, name: &str) -> Result<Team, AppError>;

    async fn delete_team(&self, requester_id: Uuid, team_id: Uuid) -> Result<(), AppError>;

    async fn add_member(
        &self,
        requester_id: Uuid,
        team_id: Uuid,
        user_id: Uuid,
        role: TeamMemberRole,
    ) -> Result<(), AppError>;

    /// Remove a member and immediately revoke their vault key share (soft
    /// revocation).  For full security in a medical context call
    /// `rotate_vault_key` afterwards on each vault the team had access to.
    async fn remove_member(
        &self,
        requester_id: Uuid,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError>;

    async fn list_user_teams(&self, user_id: Uuid) -> Result<Vec<Team>, AppError>;

    async fn list_visible_teams(&self, requester_id: Uuid) -> Result<Vec<Team>, AppError>;

    async fn list_users_for_member_picker(&self, requester_id: Uuid)
    -> Result<Vec<User>, AppError>;

    async fn list_team_members(
        &self,
        requester_id: Uuid,
        team_id: Uuid,
    ) -> Result<Vec<TeamMember>, AppError>;

    /// Grant access to a vault for a specific user (direct share, not via team).
    /// `vault_key` is the plaintext vault key (obtained via VaultService::open_vault).
    /// `recipient_master_key` is the recipient's KDF-derived master key
    ///   (available when the admin knows the password, e.g. at account creation).
    async fn grant_vault_access(
        &self,
        granter_id: Uuid,
        vault_id: Uuid,
        recipient_user_id: Uuid,
        vault_key: SecretBox<Vec<u8>>,
        recipient_master_key: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;

    /// Share a vault with all current members of a team.
    /// `vault_key` is the plaintext vault key.
    /// `member_master_keys` maps user_id → master_key for each team member.
    /// Members whose master key is absent in the map are skipped (logged as warning).
    async fn share_vault_with_team(
        &self,
        granter_id: Uuid,
        vault_id: Uuid,
        team_id: Uuid,
        vault_key: SecretBox<Vec<u8>>,
        member_master_keys: &[(Uuid, SecretBox<Vec<u8>>)],
    ) -> Result<(), AppError>;

    /// Revoke a single user's access to a vault (immediate share deletion).
    /// Call `rotate_vault_key` subsequently for complete cryptographic revocation.
    async fn revoke_vault_access(
        &self,
        actor_id: Uuid,
        vault_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError>;

    /// Full key rotation for a vault after a member is removed.
    ///
    /// Steps performed:
    /// 1. Generate a fresh vault key.
    /// 2. Atomically replace all existing key shares with new ones encrypted for
    ///    each entry in `new_shares`.
    /// 3. Update the owner's `vault_key_envelope` in the `vaults` table.
    /// 4. Write an audit entry.
    ///
    /// The caller MUST afterwards iterate every `secret_item` for this vault,
    /// decrypt each `secret_blob` with the *old* vault key, re-encrypt with
    /// `KeyRotationResult::new_vault_key`, and update the row.  Only then should
    /// the old vault key be zeroed.
    ///
    /// `new_owner_key_envelope` is the new vault key encrypted with the owner's
    /// master key (prepared by the caller).
    /// `new_shares` is a list of `(user_id, new_vault_key_encrypted_with_user_master_key,
    ///   optional_team_id)`.
    async fn rotate_vault_key(
        &self,
        actor_id: Uuid,
        vault_id: Uuid,
        new_owner_key_envelope: SecretBox<Vec<u8>>,
        new_shares: Vec<KeyShare>,
    ) -> Result<KeyRotationResult, AppError>;
}

// ── community stub ──────────────────────────────────────────────────────────

/// Community build stub: all mutating team operations return [`AppError::FeatureNotAvailable`].
pub struct CommunityTeamService;

impl TeamService for CommunityTeamService {
    async fn create_team(&self, _creator_id: Uuid, _name: &str) -> Result<Team, AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }

    async fn delete_team(&self, _requester_id: Uuid, _team_id: Uuid) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }

    async fn add_member(
        &self,
        _requester_id: Uuid,
        _team_id: Uuid,
        _user_id: Uuid,
        _role: TeamMemberRole,
    ) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }

    async fn remove_member(
        &self,
        _requester_id: Uuid,
        _team_id: Uuid,
        _user_id: Uuid,
    ) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }

    async fn list_user_teams(&self, _user_id: Uuid) -> Result<Vec<Team>, AppError> {
        Ok(vec![])
    }

    async fn list_visible_teams(&self, _requester_id: Uuid) -> Result<Vec<Team>, AppError> {
        Ok(vec![])
    }

    async fn list_users_for_member_picker(
        &self,
        _requester_id: Uuid,
    ) -> Result<Vec<User>, AppError> {
        Ok(vec![])
    }

    async fn list_team_members(
        &self,
        _requester_id: Uuid,
        _team_id: Uuid,
    ) -> Result<Vec<TeamMember>, AppError> {
        Ok(vec![])
    }

    async fn grant_vault_access(
        &self,
        _granter_id: Uuid,
        _vault_id: Uuid,
        _recipient_user_id: Uuid,
        _vault_key: SecretBox<Vec<u8>>,
        _recipient_master_key: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }

    async fn share_vault_with_team(
        &self,
        _granter_id: Uuid,
        _vault_id: Uuid,
        _team_id: Uuid,
        _vault_key: SecretBox<Vec<u8>>,
        _member_master_keys: &[(Uuid, SecretBox<Vec<u8>>)],
    ) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }

    async fn revoke_vault_access(
        &self,
        _actor_id: Uuid,
        _vault_id: Uuid,
        _user_id: Uuid,
    ) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }

    async fn rotate_vault_key(
        &self,
        _actor_id: Uuid,
        _vault_id: Uuid,
        _new_owner_key_envelope: SecretBox<Vec<u8>>,
        _new_shares: Vec<KeyShare>,
    ) -> Result<KeyRotationResult, AppError> {
        Err(AppError::FeatureNotAvailable(
            "feature-name-team-management",
        ))
    }
}

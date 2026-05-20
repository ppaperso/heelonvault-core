use std::sync::Arc;

use secrecy::SecretBox;
use tracing::{info, warn};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AuditAction, Team, TeamMember, TeamMemberRole, User, VaultShareRole};
use crate::repositories::team_repository::TeamRepository;
use crate::repositories::user_repository::UserRepository;
use crate::repositories::vault_repository::VaultRepository;
use crate::services::access_control::{check_permission, Action, Resource};
use crate::services::audit_log_service::AuditLogService;
use crate::services::crypto_service::CryptoService;

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

// ── implementation ───────────────────────────────────────────────────────────

pub struct TeamServiceImpl<TTeamRepo, TUserRepo, TVaultRepo, TCrypto, TAuditSvc>
where
    TTeamRepo: TeamRepository + Send + Sync,
    TUserRepo: UserRepository + Send + Sync,
    TVaultRepo: VaultRepository + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    team_repo: TTeamRepo,
    user_repo: TUserRepo,
    vault_repo: TVaultRepo,
    crypto_service: TCrypto,
    audit_service: Arc<TAuditSvc>,
}

impl<TTeamRepo, TUserRepo, TVaultRepo, TCrypto, TAuditSvc>
    TeamServiceImpl<TTeamRepo, TUserRepo, TVaultRepo, TCrypto, TAuditSvc>
where
    TTeamRepo: TeamRepository + Send + Sync,
    TUserRepo: UserRepository + Send + Sync,
    TVaultRepo: VaultRepository + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    pub fn new(
        team_repo: TTeamRepo,
        user_repo: TUserRepo,
        vault_repo: TVaultRepo,
        crypto_service: TCrypto,
        audit_service: Arc<TAuditSvc>,
    ) -> Self {
        Self {
            team_repo,
            user_repo,
            vault_repo,
            crypto_service,
            audit_service,
        }
    }

    async fn require_team_permission(
        &self,
        requester_id: Uuid,
        team_id: Uuid,
        action: Action,
    ) -> Result<(), AppError> {
        let user = self
            .user_repo
            .get_by_id(requester_id)
            .await?
            .ok_or_else(|| AppError::NotFound("requester not found".to_string()))?;

        let requester_role = self
            .team_repo
            .get_member_role(team_id, requester_id)
            .await?;
        check_permission(&user, action, &Resource::Team { requester_role })
    }

    fn serialize_vault_key_envelope(
        payload: &crate::services::crypto_service::EncryptedPayload,
    ) -> SecretBox<Vec<u8>> {
        use crate::services::crypto_service::NONCE_LEN;
        use secrecy::ExposeSecret;
        let mut bytes = Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
        bytes.extend_from_slice(&payload.nonce);
        bytes.extend_from_slice(payload.ciphertext.expose_secret().as_slice());
        SecretBox::new(Box::new(bytes))
    }

    fn derive_master_key_from_password_envelope(
        password_envelope: &SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        use secrecy::ExposeSecret;

        const PASSWORD_ENVELOPE_VERSION: u8 = 1;
        let bytes = password_envelope.expose_secret();

        if bytes.len() < 5 {
            return Err(AppError::Validation(
                "invalid password envelope: too short".to_string(),
            ));
        }

        if bytes[0] != PASSWORD_ENVELOPE_VERSION {
            return Err(AppError::Validation(
                "invalid password envelope: unsupported version".to_string(),
            ));
        }

        let salt_len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
        let hash_len = u16::from_be_bytes([bytes[3], bytes[4]]) as usize;
        let hash_start = 5 + salt_len;
        let expected_len = hash_start + hash_len;

        if hash_len == 0 || bytes.len() != expected_len {
            return Err(AppError::Validation(
                "invalid password envelope: malformed payload".to_string(),
            ));
        }

        Ok(SecretBox::new(Box::new(
            bytes[hash_start..expected_len].to_vec(),
        )))
    }

    async fn resolve_member_master_key(
        &self,
        team_id: Uuid,
        member_id: Uuid,
        member_master_keys: &[(Uuid, SecretBox<Vec<u8>>)],
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        use secrecy::ExposeSecret;

        if let Some((_, key)) = member_master_keys.iter().find(|(uid, _)| uid == &member_id) {
            return Ok(Some(SecretBox::new(Box::new(key.expose_secret().clone()))));
        }

        let envelope_opt = self
            .user_repo
            .get_password_envelope_by_user_id(member_id)
            .await?;

        match envelope_opt {
            Some(envelope) => match Self::derive_master_key_from_password_envelope(&envelope) {
                Ok(derived_key) => Ok(Some(derived_key)),
                Err(err) => {
                    warn!(
                        team = %team_id,
                        user = %member_id,
                        error = %err,
                        "cannot derive master key from password envelope — skipping vault share"
                    );
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    async fn insert_team_key_share(
        &self,
        granter_id: Uuid,
        vault_id: Uuid,
        team_id: Uuid,
        member_id: Uuid,
        vault_key: &SecretBox<Vec<u8>>,
        master_key: &SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        use secrecy::ExposeSecret;

        let vk_clone = SecretBox::new(Box::new(vault_key.expose_secret().clone()));
        let payload = self.crypto_service.encrypt(&vk_clone, master_key).await?;
        let envelope = Self::serialize_vault_key_envelope(&payload);
        self.vault_repo
            .insert_key_share(
                vault_id,
                member_id,
                envelope,
                Some(granter_id),
                Some(team_id),
                VaultShareRole::Read,
            )
            .await
    }

    async fn share_vault_with_members(
        &self,
        granter_id: Uuid,
        vault_id: Uuid,
        team_id: Uuid,
        vault_key: &SecretBox<Vec<u8>>,
        member_master_keys: &[(Uuid, SecretBox<Vec<u8>>)],
        member_ids: &[Uuid],
    ) -> Result<(usize, usize), AppError> {
        let mut granted = 0_usize;
        let mut skipped = 0_usize;

        for member_id in member_ids {
            let effective_key = self
                .resolve_member_master_key(team_id, *member_id, member_master_keys)
                .await?;

            if let Some(master_key) = effective_key {
                self.insert_team_key_share(
                    granter_id,
                    vault_id,
                    team_id,
                    *member_id,
                    vault_key,
                    &master_key,
                )
                .await?;
                granted += 1;
            } else {
                warn!(
                    team = %team_id,
                    user = %member_id,
                    "master key not available for member — skipping vault share"
                );
                skipped += 1;
            }
        }

        Ok((granted, skipped))
    }

    async fn record_team_share_event(
        &self,
        granter_id: Uuid,
        vault_id: Uuid,
        team_id: Uuid,
        granted: usize,
        skipped: usize,
    ) -> Result<(), AppError> {
        self.audit_service
            .record_event(
                Some(granter_id),
                AuditAction::VaultSharedWithTeam,
                Some("vault"),
                Some(&vault_id.to_string()),
                Some(&format!(
                    r#"{{"team_id":"{}","members_granted":{},"members_skipped":{}}}"#,
                    team_id, granted, skipped
                )),
            )
            .await
    }

    async fn persist_rotated_vault_keys(
        &self,
        actor_id: Uuid,
        vault_id: Uuid,
        new_owner_key_envelope: SecretBox<Vec<u8>>,
        new_shares: &[KeyShare],
    ) -> Result<(), AppError> {
        self.vault_repo
            .replace_all_key_shares(vault_id, new_shares, Some(actor_id))
            .await?;

        self.vault_repo
            .update_vault_key_envelope(vault_id, new_owner_key_envelope)
            .await
    }

    async fn record_vault_rotation_event(
        &self,
        actor_id: Uuid,
        vault_id: Uuid,
        new_share_count: usize,
    ) -> Result<(), AppError> {
        self.audit_service
            .record_event(
                Some(actor_id),
                AuditAction::VaultKeyRotated,
                Some("vault"),
                Some(&vault_id.to_string()),
                Some(&format!(r#"{{"new_share_count":{}}}"#, new_share_count)),
            )
            .await
    }
}

impl<TTeamRepo, TUserRepo, TVaultRepo, TCrypto, TAuditSvc> TeamService
    for TeamServiceImpl<TTeamRepo, TUserRepo, TVaultRepo, TCrypto, TAuditSvc>
where
    TTeamRepo: TeamRepository + Send + Sync,
    TUserRepo: UserRepository + Send + Sync,
    TVaultRepo: VaultRepository + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
{
    async fn create_team(&self, creator_id: Uuid, name: &str) -> Result<Team, AppError> {
        let _ = self
            .user_repo
            .get_by_id(creator_id)
            .await?
            .ok_or_else(|| AppError::NotFound("creator user not found".to_string()))?;

        let team_id = Uuid::new_v4();
        let team = self
            .team_repo
            .create_team(team_id, name, Some(creator_id))
            .await?;

        // Creator becomes leader automatically.
        self.team_repo
            .add_member(team_id, creator_id, &TeamMemberRole::Leader)
            .await?;

        self.audit_service
            .record_event(
                Some(creator_id),
                AuditAction::TeamCreated,
                Some("team"),
                Some(&team_id.to_string()),
                Some(&format!(r#"{{"name":"{}"}}"#, name)),
            )
            .await?;

        info!(creator = %creator_id, team = %team_id, name = name, "team created");
        Ok(team)
    }

    async fn delete_team(&self, requester_id: Uuid, team_id: Uuid) -> Result<(), AppError> {
        self.require_team_permission(requester_id, team_id, Action::TeamManageMembers)
            .await?;

        let team = self
            .team_repo
            .get_by_id(team_id)
            .await?
            .ok_or_else(|| AppError::NotFound("team not found".to_string()))?;

        self.team_repo.delete_team(team_id).await?;

        self.audit_service
            .record_event(
                Some(requester_id),
                AuditAction::TeamDeleted,
                Some("team"),
                Some(&team_id.to_string()),
                Some(&format!(r#"{{"name":"{}"}}"#, team.name)),
            )
            .await?;

        info!(actor = %requester_id, team = %team_id, "team deleted");
        Ok(())
    }

    async fn add_member(
        &self,
        requester_id: Uuid,
        team_id: Uuid,
        user_id: Uuid,
        role: TeamMemberRole,
    ) -> Result<(), AppError> {
        self.require_team_permission(requester_id, team_id, Action::TeamManageMembers)
            .await?;

        // Target user must exist.
        let _ = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user to add not found".to_string()))?;

        self.team_repo.add_member(team_id, user_id, &role).await?;

        self.audit_service
            .record_event(
                Some(requester_id),
                AuditAction::TeamMemberAdded,
                Some("team"),
                Some(&team_id.to_string()),
                Some(&format!(
                    r#"{{"user_id":"{}","role":"{}"}}"#,
                    user_id,
                    role.to_db_str()
                )),
            )
            .await?;

        info!(actor = %requester_id, team = %team_id, user = %user_id, "member added to team");
        Ok(())
    }

    async fn remove_member(
        &self,
        requester_id: Uuid,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.require_team_permission(requester_id, team_id, Action::TeamManageMembers)
            .await?;

        self.team_repo.remove_member(team_id, user_id).await?;

        let purged_share_count = self
            .vault_repo
            .delete_key_shares_for_user_via_team(user_id, team_id)
            .await?;

        self.audit_service
            .record_event(
                Some(requester_id),
                AuditAction::TeamMemberRemoved,
                Some("team"),
                Some(&team_id.to_string()),
                Some(&format!(
                    r#"{{"user_id":"{}","purged_shares":{}}}"#,
                    user_id, purged_share_count
                )),
            )
            .await?;

        info!(
            actor = %requester_id,
            team = %team_id,
            user = %user_id,
            purged_share_count = purged_share_count,
            "member removed from team and team-based key shares purged"
        );
        Ok(())
    }

    async fn list_user_teams(&self, user_id: Uuid) -> Result<Vec<Team>, AppError> {
        self.team_repo.list_for_user(user_id).await
    }

    async fn list_visible_teams(&self, requester_id: Uuid) -> Result<Vec<Team>, AppError> {
        let requester = self
            .user_repo
            .get_by_id(requester_id)
            .await?
            .ok_or_else(|| AppError::NotFound("requester not found".to_string()))?;

        if matches!(requester.role, crate::models::UserRole::Admin) {
            self.team_repo.list_all().await
        } else {
            self.team_repo.list_for_user(requester_id).await
        }
    }

    async fn list_users_for_member_picker(
        &self,
        requester_id: Uuid,
    ) -> Result<Vec<User>, AppError> {
        let _ = self
            .user_repo
            .get_by_id(requester_id)
            .await?
            .ok_or_else(|| AppError::NotFound("requester not found".to_string()))?;
        self.user_repo.list_all().await
    }

    async fn list_team_members(
        &self,
        requester_id: Uuid,
        team_id: Uuid,
    ) -> Result<Vec<TeamMember>, AppError> {
        self.require_team_permission(requester_id, team_id, Action::TeamReadMembers)
            .await?;

        self.team_repo.list_members(team_id).await
    }

    async fn grant_vault_access(
        &self,
        granter_id: Uuid,
        vault_id: Uuid,
        recipient_user_id: Uuid,
        vault_key: SecretBox<Vec<u8>>,
        recipient_master_key: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        // Encrypt vault key with recipient's master key.
        let payload = self
            .crypto_service
            .encrypt(&vault_key, &recipient_master_key)
            .await?;
        let envelope = Self::serialize_vault_key_envelope(&payload);

        self.vault_repo
            .insert_key_share(
                vault_id,
                recipient_user_id,
                envelope,
                Some(granter_id),
                None,
                VaultShareRole::Write,
            )
            .await?;

        self.audit_service
            .record_event(
                Some(granter_id),
                AuditAction::VaultSharedWithUser,
                Some("vault"),
                Some(&vault_id.to_string()),
                Some(&format!(r#"{{"recipient":"{}"}}"#, recipient_user_id)),
            )
            .await?;

        info!(
            actor = %granter_id,
            vault = %vault_id,
            recipient = %recipient_user_id,
            "vault access granted to user"
        );
        Ok(())
    }

    async fn share_vault_with_team(
        &self,
        granter_id: Uuid,
        vault_id: Uuid,
        team_id: Uuid,
        vault_key: SecretBox<Vec<u8>>,
        member_master_keys: &[(Uuid, SecretBox<Vec<u8>>)],
    ) -> Result<(), AppError> {
        let _ = self
            .team_repo
            .get_by_id(team_id)
            .await?
            .ok_or_else(|| AppError::NotFound("team not found".to_string()))?;

        let member_ids = self.team_repo.list_member_user_ids(team_id).await?;
        let (granted, skipped) = self
            .share_vault_with_members(
                granter_id,
                vault_id,
                team_id,
                &vault_key,
                member_master_keys,
                &member_ids,
            )
            .await?;
        if granted == 0 {
            return Err(AppError::Validation(
                "team share failed: no member received a vault key".to_string(),
            ));
        }

        self.record_team_share_event(granter_id, vault_id, team_id, granted, skipped)
            .await?;

        info!(
            actor = %granter_id,
            vault = %vault_id,
            team = %team_id,
            granted = granted,
            skipped = skipped,
            "vault shared with team"
        );
        Ok(())
    }

    async fn revoke_vault_access(
        &self,
        actor_id: Uuid,
        vault_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.vault_repo.delete_key_share(vault_id, user_id).await?;

        self.audit_service
            .record_event(
                Some(actor_id),
                AuditAction::VaultAccessRevoked,
                Some("vault"),
                Some(&vault_id.to_string()),
                Some(&format!(r#"{{"user_id":"{}"}}"#, user_id)),
            )
            .await?;

        info!(
            actor = %actor_id,
            vault = %vault_id,
            user = %user_id,
            "vault access revoked — call rotate_vault_key for full cryptographic revocation"
        );
        Ok(())
    }

    async fn rotate_vault_key(
        &self,
        actor_id: Uuid,
        vault_id: Uuid,
        new_owner_key_envelope: SecretBox<Vec<u8>>,
        new_shares: Vec<KeyShare>,
    ) -> Result<KeyRotationResult, AppError> {
        // Generate the new vault key (used only as a return value so the
        // caller can re-encrypt secret blobs; the actual persisted material
        // is in new_owner_key_envelope and new_shares which are already encrypted).
        let new_vault_key = generate_vault_key()?;

        self.persist_rotated_vault_keys(actor_id, vault_id, new_owner_key_envelope, &new_shares)
            .await?;

        self.record_vault_rotation_event(actor_id, vault_id, new_shares.len())
            .await?;

        info!(
            actor = %actor_id,
            vault = %vault_id,
            shares = new_shares.len(),
            "vault key rotated — caller must re-encrypt all secret items"
        );
        Ok(KeyRotationResult { new_vault_key })
    }
}

fn generate_vault_key() -> Result<SecretBox<Vec<u8>>, AppError> {
    const VAULT_KEY_LEN: usize = 32;
    let mut key = vec![0_u8; VAULT_KEY_LEN];
    getrandom::fill(key.as_mut_slice())
        .map_err(|err| AppError::Crypto(format!("vault key generation failed: {err}")))?;
    Ok(SecretBox::new(Box::new(key)))
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use std::collections::HashMap;
    use std::sync::MutexGuard;
    use std::sync::{Arc, Mutex};

    use secrecy::{ExposeSecret, SecretBox};
    use uuid::Uuid;

    use crate::errors::AppError;
    use crate::models::{
        AccessibleVault, AuditAction, Team, TeamMember, TeamMemberRole, User, UserRole, Vault,
        VaultAccessKind, VaultShareRole,
    };
    use crate::repositories::team_repository::TeamRepository;
    use crate::repositories::user_repository::UserRepository;
    use crate::repositories::vault_repository::{VaultKeyShareEnvelope, VaultRepository};
    use crate::services::audit_log_service::AuditLogService;
    use crate::services::crypto_service::CryptoServiceImpl;

    use super::{TeamService, TeamServiceImpl};

    // ── stub repos ────────────────────────────────────────────────────────────

    #[derive(Default, Clone)]
    struct StubTeamRepo {
        teams: Arc<Mutex<HashMap<Uuid, Team>>>,
        members: Arc<Mutex<Vec<TeamMember>>>,
    }

    impl StubTeamRepo {
        fn lock_teams(&self) -> Result<MutexGuard<'_, HashMap<Uuid, Team>>, AppError> {
            self.teams.lock().map_err(|_| AppError::Internal)
        }

        fn lock_members(&self) -> Result<MutexGuard<'_, Vec<TeamMember>>, AppError> {
            self.members.lock().map_err(|_| AppError::Internal)
        }
    }

    impl TeamRepository for StubTeamRepo {
        async fn create_team(
            &self,
            id: Uuid,
            name: &str,
            created_by: Option<Uuid>,
        ) -> Result<Team, AppError> {
            let team = Team {
                id,
                name: name.to_string(),
                created_by,
                created_at: "2026-01-01".to_string(),
            };
            self.lock_teams()?.insert(id, team.clone());
            Ok(team)
        }
        async fn get_by_id(&self, team_id: Uuid) -> Result<Option<Team>, AppError> {
            Ok(self.lock_teams()?.get(&team_id).cloned())
        }
        async fn list_all(&self) -> Result<Vec<Team>, AppError> {
            Ok(self.lock_teams()?.values().cloned().collect())
        }
        async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Team>, AppError> {
            let guard = self.lock_members()?;
            let team_ids: Vec<Uuid> = guard
                .iter()
                .filter(|m| m.user_id == user_id)
                .map(|m| m.team_id)
                .collect();
            drop(guard);
            let teams_guard = self.lock_teams()?;
            Ok(team_ids
                .iter()
                .filter_map(|tid| teams_guard.get(tid).cloned())
                .collect())
        }
        async fn delete_team(&self, team_id: Uuid) -> Result<(), AppError> {
            self.lock_teams()?
                .remove(&team_id)
                .ok_or_else(|| AppError::NotFound("team not found".to_string()))?;
            self.lock_members()?.retain(|m| m.team_id != team_id);
            Ok(())
        }
        async fn add_member(
            &self,
            team_id: Uuid,
            user_id: Uuid,
            role: &TeamMemberRole,
        ) -> Result<(), AppError> {
            let mut guard = self.lock_members()?;
            guard.retain(|m| !(m.team_id == team_id && m.user_id == user_id));
            guard.push(TeamMember {
                team_id,
                user_id,
                role: role.clone(),
                joined_at: "2026-01-01".to_string(),
            });
            Ok(())
        }
        async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
            let mut guard = self.lock_members()?;
            let before = guard.len();
            guard.retain(|m| !(m.team_id == team_id && m.user_id == user_id));
            if guard.len() == before {
                return Err(AppError::NotFound("membership not found".to_string()));
            }
            Ok(())
        }
        async fn list_members(&self, team_id: Uuid) -> Result<Vec<TeamMember>, AppError> {
            Ok(self
                .lock_members()?
                .iter()
                .filter(|m| m.team_id == team_id)
                .cloned()
                .collect())
        }
        async fn get_member_role(
            &self,
            team_id: Uuid,
            user_id: Uuid,
        ) -> Result<Option<TeamMemberRole>, AppError> {
            Ok(self
                .lock_members()?
                .iter()
                .find(|m| m.team_id == team_id && m.user_id == user_id)
                .map(|m| m.role.clone()))
        }
        async fn list_member_user_ids(&self, team_id: Uuid) -> Result<Vec<Uuid>, AppError> {
            Ok(self
                .lock_members()?
                .iter()
                .filter(|m| m.team_id == team_id)
                .map(|m| m.user_id)
                .collect())
        }
        async fn list_team_ids_for_user(&self, user_id: Uuid) -> Result<Vec<Uuid>, AppError> {
            Ok(self
                .lock_members()?
                .iter()
                .filter(|m| m.user_id == user_id)
                .map(|m| m.team_id)
                .collect())
        }
    }

    #[derive(Default, Clone)]
    struct StubUserRepo {
        users: Arc<Mutex<HashMap<Uuid, User>>>,
    }

    impl StubUserRepo {
        fn with_admin() -> (Self, Uuid) {
            let id = Uuid::new_v4();
            let repo = Self::default();
            if let Ok(mut users) = repo.users.lock() {
                users.insert(
                    id,
                    User {
                        id,
                        username: "admin".to_string(),
                        role: UserRole::Admin,
                        email: None,
                        display_name: None,
                        preferred_language: "fr".to_string(),
                        show_passwords_in_edit: false,
                        updated_at: None,
                    },
                );
            }
            (repo, id)
        }

        fn lock_users(&self) -> Result<MutexGuard<'_, HashMap<Uuid, User>>, AppError> {
            self.users.lock().map_err(|_| AppError::Internal)
        }
    }

    impl UserRepository for StubUserRepo {
        async fn get_by_id(&self, user_id: Uuid) -> Result<Option<User>, AppError> {
            Ok(self.lock_users()?.get(&user_id).cloned())
        }
        async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
            Ok(self
                .lock_users()?
                .values()
                .find(|u| u.username == username)
                .cloned())
        }
        async fn resolve_username_for_login_identifier(
            &self,
            identifier: &str,
        ) -> Result<Option<String>, AppError> {
            Ok(self
                .lock_users()?
                .values()
                .find(|u| u.username == identifier)
                .map(|u| u.username.clone()))
        }
        async fn list_all(&self) -> Result<Vec<User>, AppError> {
            Ok(self.lock_users()?.values().cloned().collect())
        }
        async fn create_user_db(
            &self,
            user_id: Uuid,
            username: &str,
            role: &UserRole,
        ) -> Result<(), AppError> {
            self.lock_users()?.insert(
                user_id,
                User {
                    id: user_id,
                    username: username.to_string(),
                    role: role.clone(),
                    email: None,
                    display_name: None,
                    preferred_language: "fr".to_string(),
                    show_passwords_in_edit: false,
                    updated_at: None,
                },
            );
            Ok(())
        }
        async fn delete_user(&self, user_id: Uuid) -> Result<(), AppError> {
            self.lock_users()?
                .remove(&user_id)
                .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
            Ok(())
        }
        async fn update_user_role(&self, user_id: Uuid, role: &UserRole) -> Result<(), AppError> {
            let mut guard = self.lock_users()?;
            if let Some(u) = guard.get_mut(&user_id) {
                u.role = role.clone();
                Ok(())
            } else {
                Err(AppError::NotFound("user not found".to_string()))
            }
        }
        async fn list_all_password_envelopes(&self) -> Result<Vec<(String, Vec<u8>)>, AppError> {
            Ok(vec![])
        }
        async fn get_password_envelope_by_user_id(
            &self,
            _: Uuid,
        ) -> Result<Option<secrecy::SecretBox<Vec<u8>>>, AppError> {
            Ok(None)
        }
        async fn update_user_profile(
            &self,
            _: Uuid,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<bool>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_password_envelope(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_totp_secret_envelope(
            &self,
            _: Uuid,
            _: secrecy::SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_show_passwords_in_edit(&self, _: Uuid, _: bool) -> Result<(), AppError> {
            Ok(())
        }
    }

    type TeamKeyShareMap = HashMap<(Uuid, Uuid), Vec<u8>>;

    #[derive(Default, Clone)]
    struct StubVaultRepo {
        key_shares: Arc<Mutex<TeamKeyShareMap>>,
        envelopes: Arc<Mutex<HashMap<Uuid, Vec<u8>>>>,
        vaults: Arc<Mutex<HashMap<Uuid, Vault>>>,
    }

    impl StubVaultRepo {
        fn lock_vaults(&self) -> Result<MutexGuard<'_, HashMap<Uuid, Vault>>, AppError> {
            self.vaults.lock().map_err(|_| AppError::Internal)
        }

        fn lock_envelopes(&self) -> Result<MutexGuard<'_, HashMap<Uuid, Vec<u8>>>, AppError> {
            self.envelopes.lock().map_err(|_| AppError::Internal)
        }

        fn lock_key_shares(&self) -> Result<MutexGuard<'_, TeamKeyShareMap>, AppError> {
            self.key_shares.lock().map_err(|_| AppError::Internal)
        }
    }

    impl VaultRepository for StubVaultRepo {
        async fn get_by_id(&self, vault_id: Uuid) -> Result<Option<Vault>, AppError> {
            Ok(self.lock_vaults()?.get(&vault_id).cloned())
        }
        async fn list_by_user_id(&self, _: Uuid) -> Result<Vec<Vault>, AppError> {
            Ok(vec![])
        }
        async fn list_owned_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
            self.list_by_user_id(user_id).await
        }
        async fn list_shared_vaults(&self, _: Uuid) -> Result<Vec<Vault>, AppError> {
            Ok(vec![])
        }
        async fn create_vault(&self, vault: &Vault) -> Result<(), AppError> {
            self.lock_vaults()?.insert(vault.id, vault.clone());
            Ok(())
        }
        async fn delete_vault(&self, vault_id: Uuid) -> Result<(), AppError> {
            self.lock_vaults()?.remove(&vault_id);
            Ok(())
        }
        async fn update_vault_key_envelope(
            &self,
            vault_id: Uuid,
            envelope: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            self.lock_envelopes()?
                .insert(vault_id, envelope.expose_secret().clone());
            Ok(())
        }
        async fn list_all(&self) -> Result<Vec<Vault>, AppError> {
            Ok(self.lock_vaults()?.values().cloned().collect())
        }
        async fn list_accessible_by_user(&self, _: Uuid) -> Result<Vec<Vault>, AppError> {
            Ok(vec![])
        }
        async fn get_accessible_vaults(&self, _: Uuid) -> Result<Vec<AccessibleVault>, AppError> {
            Ok(vec![])
        }
        async fn get_vault_with_permission(
            &self,
            user_id: Uuid,
            vault_id: Uuid,
        ) -> Result<Option<AccessibleVault>, AppError> {
            let guard = self.lock_vaults()?;
            let record = guard.get(&vault_id).cloned().and_then(|vault| {
                if vault.owner_user_id == user_id {
                    Some(AccessibleVault {
                        vault,
                        role: VaultShareRole::Admin,
                        access_kind: VaultAccessKind::Owner,
                        vault_key_version: 1,
                    })
                } else {
                    None
                }
            });
            Ok(record)
        }
        async fn insert_key_share(
            &self,
            vault_id: Uuid,
            user_id: Uuid,
            key_envelope: SecretBox<Vec<u8>>,
            _: Option<Uuid>,
            _: Option<Uuid>,
            _: VaultShareRole,
        ) -> Result<(), AppError> {
            self.lock_key_shares()?
                .insert((vault_id, user_id), key_envelope.expose_secret().clone());
            Ok(())
        }
        async fn get_key_share(
            &self,
            vault_id: Uuid,
            user_id: Uuid,
        ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
            Ok(self
                .lock_key_shares()?
                .get(&(vault_id, user_id))
                .map(|b| SecretBox::new(Box::new(b.clone()))))
        }
        async fn update_key_share_envelope(
            &self,
            vault_id: Uuid,
            user_id: Uuid,
            key_envelope: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            self.lock_key_shares()?
                .insert((vault_id, user_id), key_envelope.expose_secret().clone());
            Ok(())
        }
        async fn delete_key_share(&self, vault_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
            self.lock_key_shares()?.remove(&(vault_id, user_id));
            Ok(())
        }
        async fn delete_all_key_shares(&self, vault_id: Uuid) -> Result<(), AppError> {
            self.lock_key_shares()?.retain(|(v, _), _| v != &vault_id);
            Ok(())
        }
        async fn list_key_share_user_ids(&self, vault_id: Uuid) -> Result<Vec<Uuid>, AppError> {
            Ok(self
                .lock_key_shares()?
                .keys()
                .filter(|(v, _)| v == &vault_id)
                .map(|(_, u)| *u)
                .collect())
        }
        async fn replace_all_key_shares(
            &self,
            vault_id: Uuid,
            new_shares: &[VaultKeyShareEnvelope],
            _: Option<Uuid>,
        ) -> Result<(), AppError> {
            let mut guard = self.lock_key_shares()?;
            guard.retain(|(v, _), _| v != &vault_id);
            for (user_id, env, _) in new_shares {
                guard.insert((vault_id, *user_id), env.expose_secret().clone());
            }
            Ok(())
        }

        async fn delete_key_shares_for_user_via_team(
            &self,
            user_id: Uuid,
            _: Uuid,
        ) -> Result<u64, AppError> {
            let mut guard = self.lock_key_shares()?;
            let before = guard.len();
            guard.retain(|(_, u), _| *u != user_id);
            Ok((before - guard.len()) as u64)
        }

        async fn update_vault_sort_order(
            &self,
            _vault_id: Uuid,
            _sort_order: i64,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct StubAuditRepo;
    impl AuditLogService for StubAuditRepo {
        async fn record_event(
            &self,
            _: Option<Uuid>,
            _: AuditAction,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
        ) -> Result<(), AppError> {
            Ok(())
        }

        async fn list_recent(
            &self,
            _: Uuid,
            _: u32,
        ) -> Result<Vec<crate::models::AuditLogEntry>, AppError> {
            Ok(vec![])
        }
        async fn list_for_user(
            &self,
            _: Uuid,
            _: Uuid,
            _: u32,
        ) -> Result<Vec<crate::models::AuditLogEntry>, AppError> {
            Ok(vec![])
        }
        async fn list_for_target(
            &self,
            _: Uuid,
            _: &str,
            _: &str,
            _: u32,
        ) -> Result<Vec<crate::models::AuditLogEntry>, AppError> {
            Ok(vec![])
        }
    }

    fn make_service(admin_id: Uuid) -> (impl TeamService, StubTeamRepo, StubVaultRepo) {
        let (user_repo, _) = StubUserRepo::with_admin();
        // Insert the real admin_id used for tests.
        if let Ok(mut users) = user_repo.users.lock() {
            users.insert(
                admin_id,
                User {
                    id: admin_id,
                    username: "admin".to_string(),
                    role: UserRole::Admin,
                    email: None,
                    display_name: None,
                    preferred_language: "fr".to_string(),
                    show_passwords_in_edit: false,
                    updated_at: None,
                },
            );
        }
        let team_repo = StubTeamRepo::default();
        let vault_repo = StubVaultRepo::default();
        let crypto = CryptoServiceImpl::with_defaults();
        let audit = Arc::new(StubAuditRepo);

        let svc = TeamServiceImpl::new(
            team_repo.clone(),
            user_repo,
            vault_repo.clone(),
            crypto,
            audit,
        );
        (svc, team_repo, vault_repo)
    }

    #[tokio::test]
    async fn create_team_adds_creator_as_leader() {
        let admin_id = Uuid::new_v4();
        let (svc, team_repo, _) = make_service(admin_id);
        let team_result = svc.create_team(admin_id, "LabTeam").await;
        assert!(team_result.is_ok(), "create_team should succeed");
        let team = match team_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let members_result = team_repo.list_members(team.id).await;
        assert!(members_result.is_ok(), "list_members should succeed");
        let members = match members_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, admin_id);
        assert_eq!(members[0].role, TeamMemberRole::Leader);
    }

    #[tokio::test]
    async fn grant_vault_access_stores_encrypted_share() {
        let admin_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();
        let vault_id = Uuid::new_v4();

        let (svc, _, vault_repo) = make_service(admin_id);

        let vault_key = SecretBox::new(Box::new(vec![0xAA_u8; 32]));
        let master_key = SecretBox::new(Box::new(vec![0xBB_u8; 32]));

        let grant_result = svc
            .grant_vault_access(admin_id, vault_id, recipient_id, vault_key, master_key)
            .await;
        assert!(grant_result.is_ok(), "grant_vault_access should succeed");
        if grant_result.is_err() {
            return;
        }

        let share_result = vault_repo.get_key_share(vault_id, recipient_id).await;
        assert!(share_result.is_ok(), "get_key_share should succeed");
        let share = match share_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(share.is_some(), "key share should be stored");
    }

    #[tokio::test]
    async fn revoke_vault_access_removes_share() {
        let admin_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let vault_id = Uuid::new_v4();

        let (svc, _, vault_repo) = make_service(admin_id);

        let vault_key = SecretBox::new(Box::new(vec![0xAA_u8; 32]));
        let master_key = SecretBox::new(Box::new(vec![0xBB_u8; 32]));

        let grant_result = svc
            .grant_vault_access(admin_id, vault_id, user_id, vault_key, master_key)
            .await;
        assert!(grant_result.is_ok(), "grant should succeed");
        if grant_result.is_err() {
            return;
        }

        let revoke_result = svc.revoke_vault_access(admin_id, vault_id, user_id).await;
        assert!(revoke_result.is_ok(), "revoke should succeed");
        if revoke_result.is_err() {
            return;
        }

        let share_result = vault_repo.get_key_share(vault_id, user_id).await;
        assert!(share_result.is_ok(), "get_key_share should succeed");
        let share = match share_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(
            share.is_none(),
            "key share should be deleted after revocation"
        );
    }
}

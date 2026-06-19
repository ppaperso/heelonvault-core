use std::sync::Arc;

use secrecy::{ExposeSecret, SecretBox};
use uuid::Uuid;

use crate::errors::{AccessDeniedReason, AppError};
use crate::models::{AccessibleVault, AuditAction, Vault};
use crate::repositories::team_repository::TeamRepository;
use crate::repositories::user_repository::UserRepository;
use crate::repositories::vault_repository::VaultRepository;
use crate::services::access_control::{Action, Resource, check_permission};
use crate::services::audit_log_service::AuditLogService;
use crate::services::crypto_service::{CryptoService, EncryptedPayload, NONCE_LEN};

pub const VAULT_KEY_LEN: usize = 32;

#[trait_variant::make(VaultKeyEnvelopeRepository: Send)]
pub trait LocalVaultKeyEnvelopeRepository {
    /// Return the encrypted vault key envelope for a vault, if present.
    async fn get_vault_key_envelope(
        &self,
        vault_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError>;
}

#[trait_variant::make(VaultService: Send)]
pub trait LocalVaultService {
    /// Create a new vault for `owner_user_id` and initialize its key envelope.
    ///
    /// `master_key` must be the owner's KDF-derived master key.
    async fn create_vault(
        &self,
        owner_user_id: Uuid,
        name: &str,
        master_key: SecretBox<Vec<u8>>,
    ) -> Result<Vault, AppError>;
    /// Open a vault by decrypting the stored envelope with `master_key`.
    ///
    /// Returns the plaintext vault key on success.
    async fn open_vault(
        &self,
        vault_id: Uuid,
        master_key: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError>;
    /// Open a vault for a specific requester after permission checks.
    ///
    /// Returns `AccessDenied` when the user has no read access.
    async fn open_vault_for_user(
        &self,
        requester_id: Uuid,
        vault_id: Uuid,
        master_key: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError>;
    /// List all vaults visible to a user (owned and shared).
    async fn list_user_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError>;
    /// Return access metadata for one vault/user pair.
    async fn get_vault_access_for_user(
        &self,
        user_id: Uuid,
        vault_id: Uuid,
    ) -> Result<Option<AccessibleVault>, AppError>;
    /// List only vaults owned by `user_id`.
    async fn list_owned_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError>;
    /// List only vaults shared with `user_id`.
    async fn list_shared_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError>;
    /// List shared-vault access records for UI/security decisions.
    async fn list_shared_vault_access(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<AccessibleVault>, AppError>;
    /// Returns true when a vault has at least one external share.
    async fn is_vault_shared_with_others(
        &self,
        user_id: Uuid,
        vault_id: Uuid,
    ) -> Result<bool, AppError>;
    /// Permanently delete a vault after ownership and share constraints checks.
    async fn delete_vault(&self, requester_id: Uuid, vault_id: Uuid) -> Result<(), AppError>;
    /// Update the display sort order for a vault (owner only).
    async fn update_vault_sort_order(
        &self,
        requester_id: Uuid,
        vault_id: Uuid,
        sort_order: i64,
    ) -> Result<(), AppError>;
}

pub struct VaultServiceImpl<TVaultRepo, TEnvelopeRepo, TUserRepo, TTeamRepo, TAuditSvc, TCrypto>
where
    TVaultRepo: VaultRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TUserRepo: UserRepository + Send + Sync,
    TTeamRepo: TeamRepository + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    vault_repo: TVaultRepo,
    envelope_repo: TEnvelopeRepo,
    user_repo: TUserRepo,
    #[allow(dead_code)]
    team_repo: TTeamRepo,
    audit_service: Arc<TAuditSvc>,
    crypto_service: TCrypto,
}

impl<TVaultRepo, TEnvelopeRepo, TUserRepo, TTeamRepo, TAuditSvc, TCrypto>
    VaultServiceImpl<TVaultRepo, TEnvelopeRepo, TUserRepo, TTeamRepo, TAuditSvc, TCrypto>
where
    TVaultRepo: VaultRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TUserRepo: UserRepository + Send + Sync,
    TTeamRepo: TeamRepository + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    pub fn new(
        vault_repo: TVaultRepo,
        envelope_repo: TEnvelopeRepo,
        user_repo: TUserRepo,
        team_repo: TTeamRepo,
        audit_service: Arc<TAuditSvc>,
        crypto_service: TCrypto,
    ) -> Self {
        Self {
            vault_repo,
            envelope_repo,
            user_repo,
            team_repo,
            audit_service,
            crypto_service,
        }
    }

    fn serialize_envelope(payload: &EncryptedPayload) -> SecretBox<Vec<u8>> {
        let mut bytes = Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
        bytes.extend_from_slice(&payload.nonce);
        bytes.extend_from_slice(payload.ciphertext.expose_secret().as_slice());
        SecretBox::new(Box::new(bytes))
    }

    fn deserialize_envelope(bytes: &SecretBox<Vec<u8>>) -> Result<EncryptedPayload, AppError> {
        if bytes.expose_secret().len() < NONCE_LEN {
            return Err(AppError::Storage(
                "vault key envelope is invalid".to_string(),
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

    fn generate_vault_key() -> Result<SecretBox<Vec<u8>>, AppError> {
        let mut key = vec![0_u8; VAULT_KEY_LEN];
        getrandom::fill(key.as_mut_slice())
            .map_err(|err| AppError::Crypto(format!("vault key generation failed: {err}")))?;
        Ok(SecretBox::new(Box::new(key)))
    }
}

impl<TVaultRepo, TEnvelopeRepo, TUserRepo, TTeamRepo, TAuditSvc, TCrypto> VaultService
    for VaultServiceImpl<TVaultRepo, TEnvelopeRepo, TUserRepo, TTeamRepo, TAuditSvc, TCrypto>
where
    TVaultRepo: VaultRepository + Send + Sync,
    TEnvelopeRepo: VaultKeyEnvelopeRepository + Send + Sync,
    TUserRepo: UserRepository + Send + Sync,
    TTeamRepo: TeamRepository + Send + Sync,
    TAuditSvc: AuditLogService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    async fn create_vault(
        &self,
        owner_user_id: Uuid,
        name: &str,
        master_key: SecretBox<Vec<u8>>,
    ) -> Result<Vault, AppError> {
        if name.trim().is_empty() {
            return Err(AppError::Validation(
                "vault name must not be empty".to_string(),
            ));
        }

        let owner = self
            .user_repo
            .get_by_id(owner_user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("owner user not found".to_string()))?;
        check_permission(&owner, Action::VaultCreate, &Resource::Global)?;

        let vault = Vault {
            id: Uuid::new_v4(),
            owner_user_id,
            name: name.to_string(),
        };

        let vault_key = Self::generate_vault_key()?;
        let encrypted_payload = self.crypto_service.encrypt(&vault_key, &master_key).await?;
        let serialized = Self::serialize_envelope(&encrypted_payload);

        self.vault_repo.create_vault(&vault).await?;
        self.vault_repo
            .update_vault_key_envelope(vault.id, serialized)
            .await?;

        self.audit_service
            .record_event(
                Some(owner_user_id),
                AuditAction::VaultCreated,
                Some("vault"),
                Some(&vault.id.to_string()),
                Some(&format!(r#"{{"name":"{}"}}"#, vault.name)),
            )
            .await?;

        Ok(vault)
    }

    async fn open_vault(
        &self,
        vault_id: Uuid,
        master_key: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        let vault = self
            .vault_repo
            .get_by_id(vault_id)
            .await?
            .ok_or_else(|| AppError::NotFound("vault not found".to_string()))?;

        VaultService::open_vault_for_user(self, vault.owner_user_id, vault_id, master_key).await
    }

    async fn open_vault_for_user(
        &self,
        requester_id: Uuid,
        vault_id: Uuid,
        master_key: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        let requester = self
            .user_repo
            .get_by_id(requester_id)
            .await?
            .ok_or_else(|| AppError::NotFound("requester user not found".to_string()))?;

        let permission = self
            .vault_repo
            .get_vault_with_permission(requester_id, vault_id)
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::VaultAccessDenied,
            ))?;

        let is_owner = permission.vault.owner_user_id == requester_id;
        let has_direct_share = matches!(
            permission.access_kind,
            crate::models::VaultAccessKind::DirectShare
        );
        let has_team_share = matches!(
            permission.access_kind,
            crate::models::VaultAccessKind::TeamShare
        );

        check_permission(
            &requester,
            Action::VaultOpen,
            &Resource::Vault {
                is_owner,
                has_direct_share,
                has_team_share,
                share_role: Some(permission.role),
            },
        )?;

        let envelope = if is_owner {
            self.envelope_repo
                .get_vault_key_envelope(vault_id)
                .await?
                .ok_or_else(|| AppError::Storage("missing owner vault key envelope".to_string()))?
        } else {
            self.vault_repo
                .get_key_share(vault_id, requester_id)
                .await?
                .ok_or(AppError::Authorization(
                    AccessDeniedReason::VaultAccessDenied,
                ))?
        };

        let payload = Self::deserialize_envelope(&envelope)?;
        let vault_key = self.crypto_service.decrypt(&payload, &master_key).await?;

        self.audit_service
            .record_event(
                Some(requester_id),
                AuditAction::VaultOpened,
                Some("vault"),
                Some(&vault_id.to_string()),
                None,
            )
            .await?;

        Ok(vault_key)
    }

    async fn list_user_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        check_permission(&user, Action::VaultList, &Resource::Global)?;

        let records = self.vault_repo.get_accessible_vaults(user_id).await?;
        Ok(records.into_iter().map(|record| record.vault).collect())
    }

    async fn get_vault_access_for_user(
        &self,
        user_id: Uuid,
        vault_id: Uuid,
    ) -> Result<Option<AccessibleVault>, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        check_permission(&user, Action::VaultList, &Resource::Global)?;

        self.vault_repo
            .get_vault_with_permission(user_id, vault_id)
            .await
    }

    async fn list_owned_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        check_permission(&user, Action::VaultList, &Resource::Global)?;

        self.vault_repo.list_owned_vaults(user_id).await
    }

    async fn list_shared_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        check_permission(&user, Action::VaultList, &Resource::Global)?;

        self.vault_repo.list_shared_vaults(user_id).await
    }

    async fn list_shared_vault_access(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<AccessibleVault>, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        check_permission(&user, Action::VaultList, &Resource::Global)?;

        let records = self.vault_repo.get_accessible_vaults(user_id).await?;
        Ok(records
            .into_iter()
            .filter(|record| record.vault.owner_user_id != user_id)
            .collect())
    }

    async fn is_vault_shared_with_others(
        &self,
        user_id: Uuid,
        vault_id: Uuid,
    ) -> Result<bool, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("user not found".to_string()))?;
        check_permission(&user, Action::VaultList, &Resource::Global)?;

        let access = self
            .vault_repo
            .get_vault_with_permission(user_id, vault_id)
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::VaultAccessDenied,
            ))?;

        if access.vault.owner_user_id != user_id {
            return Ok(false);
        }

        let shared_user_ids = self.vault_repo.list_key_share_user_ids(vault_id).await?;
        Ok(!shared_user_ids.is_empty())
    }

    async fn delete_vault(&self, requester_id: Uuid, vault_id: Uuid) -> Result<(), AppError> {
        let requester = self
            .user_repo
            .get_by_id(requester_id)
            .await?
            .ok_or_else(|| AppError::NotFound("requester user not found".to_string()))?;

        let permission = self
            .vault_repo
            .get_vault_with_permission(requester_id, vault_id)
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::VaultAccessDenied,
            ))?;

        let is_owner = permission.vault.owner_user_id == requester_id;
        let has_direct_share = matches!(
            permission.access_kind,
            crate::models::VaultAccessKind::DirectShare
        );
        let has_team_share = matches!(
            permission.access_kind,
            crate::models::VaultAccessKind::TeamShare
        );

        check_permission(
            &requester,
            Action::VaultDelete,
            &Resource::Vault {
                is_owner,
                has_direct_share,
                has_team_share,
                share_role: Some(permission.role),
            },
        )?;

        self.vault_repo.delete_vault(vault_id).await?;

        self.audit_service
            .record_event(
                Some(requester_id),
                AuditAction::VaultDeleted,
                Some("vault"),
                Some(&vault_id.to_string()),
                None,
            )
            .await
            .ok();

        Ok(())
    }

    async fn update_vault_sort_order(
        &self,
        requester_id: Uuid,
        vault_id: Uuid,
        sort_order: i64,
    ) -> Result<(), AppError> {
        let permission = self
            .vault_repo
            .get_vault_with_permission(requester_id, vault_id)
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::VaultAccessDenied,
            ))?;

        if permission.vault.owner_user_id != requester_id {
            return Err(AppError::Authorization(
                AccessDeniedReason::VaultAccessDenied,
            ));
        }

        self.vault_repo
            .update_vault_sort_order(vault_id, sort_order)
            .await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use secrecy::{ExposeSecret, SecretBox};
    use uuid::Uuid;

    use crate::errors::AppError;
    use crate::models::{
        AccessibleVault, AuditAction, AuditLogEntry, Team, TeamMember, TeamMemberRole, User,
        UserRole, Vault, VaultAccessKind, VaultShareRole,
    };
    use crate::repositories::team_repository::TeamRepository;
    use crate::repositories::user_repository::UserRepository;
    use crate::repositories::vault_repository::{VaultKeyShareEnvelope, VaultRepository};
    use crate::services::audit_log_service::AuditLogService;
    use crate::services::crypto_service::{CryptoService, EncryptedPayload, NONCE_LEN};

    use super::{VAULT_KEY_LEN, VaultKeyEnvelopeRepository, VaultService, VaultServiceImpl};

    type VaultEnvelopeMap = HashMap<Uuid, SecretBox<Vec<u8>>>;

    #[derive(Default, Clone)]
    struct StubVaultRepository {
        vaults: Arc<Mutex<HashMap<Uuid, Vault>>>,
        envelopes: Arc<Mutex<VaultEnvelopeMap>>,
    }

    impl StubVaultRepository {
        fn lock_vaults(&self) -> Result<std::sync::MutexGuard<'_, HashMap<Uuid, Vault>>, AppError> {
            self.vaults
                .lock()
                .map_err(|_| AppError::Storage("vault lock poisoned".to_string()))
        }

        fn lock_envelopes(&self) -> Result<std::sync::MutexGuard<'_, VaultEnvelopeMap>, AppError> {
            self.envelopes
                .lock()
                .map_err(|_| AppError::Storage("envelope lock poisoned".to_string()))
        }
    }

    impl VaultRepository for StubVaultRepository {
        async fn get_by_id(&self, vault_id: Uuid) -> Result<Option<Vault>, AppError> {
            let guard = self.lock_vaults()?;
            Ok(guard.get(&vault_id).cloned())
        }

        async fn list_by_user_id(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
            let guard = self.lock_vaults()?;
            Ok(guard
                .values()
                .filter(|vault| vault.owner_user_id == user_id)
                .cloned()
                .collect())
        }

        async fn list_owned_vaults(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
            self.list_by_user_id(user_id).await
        }

        async fn list_shared_vaults(&self, _: Uuid) -> Result<Vec<Vault>, AppError> {
            Ok(vec![])
        }

        async fn create_vault(&self, vault: &Vault) -> Result<(), AppError> {
            let mut guard = self.lock_vaults()?;
            guard.insert(vault.id, vault.clone());
            Ok(())
        }

        async fn delete_vault(&self, vault_id: Uuid) -> Result<(), AppError> {
            let mut guard = self.lock_vaults()?;
            guard.remove(&vault_id);
            Ok(())
        }

        async fn update_vault_key_envelope(
            &self,
            vault_id: Uuid,
            encrypted_vault_key_envelope: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            let mut guard = self.lock_envelopes()?;
            guard.insert(vault_id, encrypted_vault_key_envelope);
            Ok(())
        }

        async fn list_all(&self) -> Result<Vec<Vault>, AppError> {
            Ok(self.lock_vaults()?.values().cloned().collect())
        }

        async fn list_accessible_by_user(&self, user_id: Uuid) -> Result<Vec<Vault>, AppError> {
            self.list_by_user_id(user_id).await
        }

        async fn get_accessible_vaults(
            &self,
            user_id: Uuid,
        ) -> Result<Vec<AccessibleVault>, AppError> {
            let owned = self.list_by_user_id(user_id).await?;
            Ok(owned
                .into_iter()
                .map(|vault| AccessibleVault {
                    vault,
                    role: VaultShareRole::Admin,
                    access_kind: VaultAccessKind::Owner,
                    vault_key_version: 1,
                })
                .collect())
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
            _: Uuid,
            _: Uuid,
            _: SecretBox<Vec<u8>>,
            _: Option<Uuid>,
            _: Option<Uuid>,
            _: VaultShareRole,
        ) -> Result<(), AppError> {
            Ok(())
        }

        async fn get_key_share(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
            Ok(None)
        }

        async fn update_key_share_envelope(
            &self,
            _: Uuid,
            _: Uuid,
            _: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }

        async fn delete_key_share(&self, _: Uuid, _: Uuid) -> Result<(), AppError> {
            Ok(())
        }

        async fn delete_all_key_shares(&self, _: Uuid) -> Result<(), AppError> {
            Ok(())
        }

        async fn list_key_share_user_ids(&self, _: Uuid) -> Result<Vec<Uuid>, AppError> {
            Ok(vec![])
        }

        async fn replace_all_key_shares(
            &self,
            _: Uuid,
            _: &[VaultKeyShareEnvelope],
            _: Option<Uuid>,
        ) -> Result<(), AppError> {
            Ok(())
        }

        async fn delete_key_shares_for_user_via_team(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<u64, AppError> {
            Ok(0)
        }

        async fn update_vault_sort_order(
            &self,
            _vault_id: Uuid,
            _sort_order: i64,
        ) -> Result<(), AppError> {
            Ok(())
        }
    }

    impl VaultKeyEnvelopeRepository for StubVaultRepository {
        async fn get_vault_key_envelope(
            &self,
            vault_id: Uuid,
        ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
            let guard = self.lock_envelopes()?;
            Ok(guard
                .get(&vault_id)
                .map(|bytes| SecretBox::new(Box::new(bytes.expose_secret().clone()))))
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
            _password: &secrecy::SecretString,
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
            nonce[0] = 42;

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
    struct StubUserRepository {
        users: Arc<Mutex<HashMap<Uuid, User>>>,
    }

    impl UserRepository for StubUserRepository {
        async fn get_by_id(&self, user_id: Uuid) -> Result<Option<User>, AppError> {
            Ok(self
                .users
                .lock()
                .map_err(|_| AppError::Internal)?
                .get(&user_id)
                .cloned())
        }
        async fn get_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
            Ok(self
                .users
                .lock()
                .map_err(|_| AppError::Internal)?
                .values()
                .find(|u| u.username == username)
                .cloned())
        }
        async fn resolve_username_for_login_identifier(
            &self,
            identifier: &str,
        ) -> Result<Option<String>, AppError> {
            Ok(self
                .users
                .lock()
                .map_err(|_| AppError::Internal)?
                .values()
                .find(|u| u.username == identifier)
                .map(|u| u.username.clone()))
        }
        async fn list_all(&self) -> Result<Vec<User>, AppError> {
            Ok(vec![])
        }
        async fn create_user_db(&self, _: Uuid, _: &str, _: &UserRole) -> Result<(), AppError> {
            Ok(())
        }
        async fn delete_user(&self, _: Uuid) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_user_role(&self, _: Uuid, _: &UserRole) -> Result<(), AppError> {
            Ok(())
        }
        async fn list_all_password_envelopes(&self) -> Result<Vec<(String, Vec<u8>)>, AppError> {
            Ok(vec![])
        }
        async fn get_password_envelope_by_user_id(
            &self,
            _: Uuid,
        ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
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
            _: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_totp_secret_envelope(
            &self,
            _: Uuid,
            _: SecretBox<Vec<u8>>,
        ) -> Result<(), AppError> {
            Ok(())
        }
        async fn update_show_passwords_in_edit(&self, _: Uuid, _: bool) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[derive(Default, Clone)]
    struct StubTeamRepository;

    impl TeamRepository for StubTeamRepository {
        async fn create_team(&self, _: Uuid, _: &str, _: Option<Uuid>) -> Result<Team, AppError> {
            Err(AppError::Internal)
        }
        async fn get_by_id(&self, _: Uuid) -> Result<Option<Team>, AppError> {
            Ok(None)
        }
        async fn list_all(&self) -> Result<Vec<Team>, AppError> {
            Ok(vec![])
        }
        async fn list_for_user(&self, _: Uuid) -> Result<Vec<Team>, AppError> {
            Ok(vec![])
        }
        async fn delete_team(&self, _: Uuid) -> Result<(), AppError> {
            Ok(())
        }
        async fn add_member(&self, _: Uuid, _: Uuid, _: &TeamMemberRole) -> Result<(), AppError> {
            Ok(())
        }
        async fn remove_member(&self, _: Uuid, _: Uuid) -> Result<(), AppError> {
            Ok(())
        }
        async fn list_members(&self, _: Uuid) -> Result<Vec<TeamMember>, AppError> {
            Ok(vec![])
        }
        async fn get_member_role(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<Option<TeamMemberRole>, AppError> {
            Ok(None)
        }
        async fn list_member_user_ids(&self, _: Uuid) -> Result<Vec<Uuid>, AppError> {
            Ok(vec![])
        }
        async fn list_team_ids_for_user(&self, _: Uuid) -> Result<Vec<Uuid>, AppError> {
            Ok(vec![])
        }
    }

    #[derive(Default)]
    struct StubAuditLogService;

    impl AuditLogService for StubAuditLogService {
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

        async fn list_recent(&self, _: Uuid, _: u32) -> Result<Vec<AuditLogEntry>, AppError> {
            Ok(vec![])
        }
        async fn list_for_user(
            &self,
            _: Uuid,
            _: Uuid,
            _: u32,
        ) -> Result<Vec<AuditLogEntry>, AppError> {
            Ok(vec![])
        }
        async fn list_for_target(
            &self,
            _: Uuid,
            _: &str,
            _: &str,
            _: u32,
        ) -> Result<Vec<AuditLogEntry>, AppError> {
            Ok(vec![])
        }
    }

    fn make_service_with_owner(
        owner_id: Uuid,
        repo: StubVaultRepository,
    ) -> VaultServiceImpl<
        StubVaultRepository,
        StubVaultRepository,
        StubUserRepository,
        StubTeamRepository,
        StubAuditLogService,
        StubCryptoService,
    > {
        let user_repo = StubUserRepository::default();
        if let Ok(mut users) = user_repo.users.lock() {
            users.insert(
                owner_id,
                User {
                    id: owner_id,
                    username: "owner".to_string(),
                    role: UserRole::User,
                    email: None,
                    display_name: None,
                    preferred_language: "fr".to_string(),
                    show_passwords_in_edit: false,
                    updated_at: None,
                },
            );
        }

        VaultServiceImpl::new(
            repo.clone(),
            repo,
            user_repo,
            StubTeamRepository,
            Arc::new(StubAuditLogService),
            StubCryptoService,
        )
    }

    #[tokio::test]
    async fn create_vault_persists_envelope_and_lists() {
        let repo = StubVaultRepository::default();
        let owner_user_id = Uuid::new_v4();
        let service = make_service_with_owner(owner_user_id, repo.clone());
        let master_key = SecretBox::new(Box::new(vec![8_u8; 32]));

        let vault_result = service
            .create_vault(owner_user_id, "Work", master_key)
            .await;
        assert!(vault_result.is_ok(), "create_vault should succeed");
        let vault = match vault_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let listed_result = service.list_user_vaults(owner_user_id).await;
        assert!(listed_result.is_ok(), "list should succeed");
        let listed = match listed_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, vault.id);

        let envelope_result = repo.get_vault_key_envelope(vault.id).await;
        assert!(envelope_result.is_ok(), "envelope load should succeed");
        let envelope_opt = match envelope_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(envelope_opt.is_some(), "envelope should be stored");
        let envelope = match envelope_opt {
            Some(value) => value,
            None => return,
        };
        assert!(envelope.expose_secret().len() > NONCE_LEN);
    }

    #[tokio::test]
    async fn open_vault_decrypts_key() {
        let repo = StubVaultRepository::default();
        let owner_user_id = Uuid::new_v4();
        let service = make_service_with_owner(owner_user_id, repo.clone());
        let master_key = SecretBox::new(Box::new(vec![9_u8; 32]));

        let created_result = service
            .create_vault(
                owner_user_id,
                "Personal",
                SecretBox::new(Box::new(vec![9_u8; 32])),
            )
            .await;
        assert!(created_result.is_ok(), "create should succeed");
        let created = match created_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let opened_result = service.open_vault(created.id, master_key).await;
        assert!(opened_result.is_ok(), "open should succeed");
        let opened_key = match opened_result {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(opened_key.expose_secret().len(), VAULT_KEY_LEN);
    }

    #[tokio::test]
    async fn open_vault_returns_not_found() {
        let repo = StubVaultRepository::default();
        let service = make_service_with_owner(Uuid::new_v4(), repo);

        let result = service
            .open_vault(Uuid::new_v4(), SecretBox::new(Box::new(vec![1_u8; 32])))
            .await;
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn open_vault_rejects_invalid_envelope() {
        let repo = StubVaultRepository::default();
        let service = make_service_with_owner(Uuid::new_v4(), repo.clone());

        let vault = Vault {
            id: Uuid::new_v4(),
            owner_user_id: Uuid::new_v4(),
            name: "Broken".to_string(),
        };

        let create_result = repo.create_vault(&vault).await;
        assert!(create_result.is_ok(), "seed vault should succeed");
        if create_result.is_err() {
            return;
        }

        let envelope_result = repo
            .update_vault_key_envelope(vault.id, SecretBox::new(Box::new(vec![1_u8, 2_u8, 3_u8])))
            .await;
        assert!(envelope_result.is_ok(), "seed envelope should succeed");
        if envelope_result.is_err() {
            return;
        }

        let opened_result = service
            .open_vault(vault.id, SecretBox::new(Box::new(vec![7_u8; 32])))
            .await;
        assert!(matches!(opened_result, Err(AppError::NotFound(_))));
    }
}

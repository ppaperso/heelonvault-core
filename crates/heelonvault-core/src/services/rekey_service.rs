use secrecy::{ExposeSecret, SecretBox, SecretString};
use subtle::ConstantTimeEq;
use tracing::warn;
use uuid::Uuid;

use crate::errors::{AppError, RecoveryFailure};
use crate::repositories::secret_repository::SecretRepository;
use crate::repositories::user_repository::{UserKeyMaterialRepository, UserRepository};
use crate::repositories::vault_repository::{
    MasterKeyRotationRepository, UserEnvelopeChange, UserKeyMaterialUpdate, VaultEnvelopeUpdate,
    VaultRepository, VaultShareEnvelopeUpdate,
};
use crate::services::account_key::{generate_account_key, seal_with_recovery_phrase};
use crate::services::auth_service::{PasswordEnvelope, encode_password_envelope, wrap_account_key};
use crate::services::crypto_service::{
    CryptoService, EncryptedPayload, decode_envelope, encode_envelope,
};
use crate::services::vault_service::{
    VaultKeyEnvelopeRepository, deserialize_vault_key_envelope, serialize_vault_key_envelope,
};

pub struct RekeyRepositories<'a, U, V, E, S> {
    pub users: &'a U,
    pub vaults: &'a V,
    pub envelopes: &'a E,
    pub secrets: &'a S,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RekeyReport {
    pub owner_vaults_rewrapped: usize,
    pub shared_vaults_rewrapped: usize,
    /// Vaults stored without a key envelope: unopenable before, left untouched.
    pub vaults_without_key: usize,
    pub secrets_readable: usize,
    pub secrets_unreadable: usize,
    /// The TOTP secret no longer matched the previous key and was removed.
    pub totp_disabled: bool,
    /// The recovery phrase opens the account afterwards.
    pub recovery_key_available: bool,
}

pub struct RekeyOutcome {
    pub report: RekeyReport,
    pub password_envelope: SecretBox<Vec<u8>>,
    /// Key the running session uses from now on.
    pub session_key: SecretBox<Vec<u8>>,
}

enum RecoveryMaterial<'a> {
    /// Multi-account vaults: the stored phrase follows the new master key.
    LegacyPhraseEnvelope,
    /// The account key is sealed with the phrase, read from the legacy envelope when not given.
    SealedAccountKey(Option<&'a SecretString>),
}

struct UnwrappedVault {
    vault_id: Uuid,
    owned: bool,
    vault_key: SecretBox<Vec<u8>>,
}

#[derive(Clone, Copy)]
enum BlobFormat {
    /// `version || nonce || ciphertext` (legacy recovery phrase envelope).
    Versioned,
    /// `nonce || ciphertext` (vault keys, TOTP secret, secret blobs).
    NonceCiphertext,
}

impl BlobFormat {
    fn decode(self, bytes: &SecretBox<Vec<u8>>) -> Result<EncryptedPayload, AppError> {
        match self {
            Self::Versioned => decode_envelope(bytes.expose_secret()),
            Self::NonceCiphertext => deserialize_vault_key_envelope(bytes),
        }
    }

    fn encode(self, payload: &EncryptedPayload) -> SecretBox<Vec<u8>> {
        match self {
            Self::Versioned => encode_envelope(payload),
            Self::NonceCiphertext => serialize_vault_key_envelope(payload),
        }
    }
}

/// Moves a legacy account to a random account key protected by `password`. Vault keys, the TOTP
/// secret and the recovery material are rewrapped in one transaction; secrets are not rewritten.
pub async fn migrate_to_account_key<U, V, E, S, C>(
    repos: RekeyRepositories<'_, U, V, E, S>,
    crypto: &C,
    user_id: Uuid,
    legacy_master_key: &SecretBox<Vec<u8>>,
    password: &SecretString,
    recovery_phrase: Option<&SecretString>,
) -> Result<RekeyOutcome, AppError>
where
    U: UserRepository + UserKeyMaterialRepository + Send + Sync,
    V: VaultRepository + MasterKeyRotationRepository + Send + Sync,
    E: VaultKeyEnvelopeRepository + Send + Sync,
    S: SecretRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    let account_key = generate_account_key()?;
    let envelope = wrap_account_key(crypto, password, &account_key).await?;
    rewrap_account(
        repos,
        crypto,
        user_id,
        legacy_master_key,
        account_key,
        &envelope,
        RecoveryMaterial::SealedAccountKey(recovery_phrase),
    )
    .await
}

/// New password for an account of a multi-account vault, which keeps the legacy format until
/// accounts can share vaults without reading each other's key.
pub async fn rekey_legacy_account<U, V, E, S, C>(
    repos: RekeyRepositories<'_, U, V, E, S>,
    crypto: &C,
    user_id: Uuid,
    legacy_master_key: &SecretBox<Vec<u8>>,
    new_password: &SecretString,
) -> Result<RekeyOutcome, AppError>
where
    U: UserRepository + UserKeyMaterialRepository + Send + Sync,
    V: VaultRepository + MasterKeyRotationRepository + Send + Sync,
    E: VaultKeyEnvelopeRepository + Send + Sync,
    S: SecretRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    let salt = crypto.generate_kdf_salt().await?;
    let master_key = crypto.derive_key(new_password, &salt).await?;
    let envelope = PasswordEnvelope::Legacy {
        salt,
        master_key: copy_secret(&master_key),
    };
    rewrap_account(
        repos,
        crypto,
        user_id,
        legacy_master_key,
        master_key,
        &envelope,
        RecoveryMaterial::LegacyPhraseEnvelope,
    )
    .await
}

/// New password for an account-key account. The account key does not change, so only the
/// password envelope is rewritten; every vault must open with `account_key` first.
pub async fn replace_password<U, V, E, S, C>(
    repos: RekeyRepositories<'_, U, V, E, S>,
    crypto: &C,
    user_id: Uuid,
    account_key: &SecretBox<Vec<u8>>,
    new_password: &SecretString,
) -> Result<RekeyOutcome, AppError>
where
    U: UserRepository + UserKeyMaterialRepository + Send + Sync,
    V: VaultRepository + MasterKeyRotationRepository + Send + Sync,
    E: VaultKeyEnvelopeRepository + Send + Sync,
    S: SecretRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    let (vaults, vaults_without_key) =
        unwrap_accessible_vaults(&repos, crypto, user_id, account_key).await?;
    let (secrets_readable, secrets_unreadable) =
        check_secrets(repos.secrets, crypto, &vaults).await?;

    let password_envelope =
        encode_password_envelope(&wrap_account_key(crypto, new_password, account_key).await?);
    repos
        .users
        .update_password_envelope(user_id, copy_secret(&password_envelope))
        .await?;
    verify_stored_envelope(repos.users, user_id, &password_envelope).await?;

    let report = RekeyReport {
        vaults_without_key,
        secrets_readable,
        secrets_unreadable,
        recovery_key_available: repos
            .users
            .get_recovery_key_envelope(user_id)
            .await?
            .is_some(),
        ..RekeyReport::default()
    };
    Ok(RekeyOutcome {
        report,
        password_envelope,
        session_key: copy_secret(account_key),
    })
}

fn copy_secret(secret: &SecretBox<Vec<u8>>) -> SecretBox<Vec<u8>> {
    SecretBox::new(Box::new(secret.expose_secret().clone()))
}

/// Rewraps everything `old_key` protects under `new_key`, in one transaction, then re-reads it.
/// Nothing is written unless every vault opens with `old_key`.
async fn rewrap_account<U, V, E, S, C>(
    repos: RekeyRepositories<'_, U, V, E, S>,
    crypto: &C,
    user_id: Uuid,
    old_key: &SecretBox<Vec<u8>>,
    new_key: SecretBox<Vec<u8>>,
    envelope: &PasswordEnvelope,
    recovery: RecoveryMaterial<'_>,
) -> Result<RekeyOutcome, AppError>
where
    U: UserRepository + UserKeyMaterialRepository + Send + Sync,
    V: VaultRepository + MasterKeyRotationRepository + Send + Sync,
    E: VaultKeyEnvelopeRepository + Send + Sync,
    S: SecretRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    let (vaults, vaults_without_key) =
        unwrap_accessible_vaults(&repos, crypto, user_id, old_key).await?;
    let (secrets_readable, secrets_unreadable) =
        check_secrets(repos.secrets, crypto, &vaults).await?;

    let mut owner_updates = Vec::new();
    let mut shared_updates = Vec::new();
    for vault in &vaults {
        let key_envelope =
            serialize_vault_key_envelope(&crypto.encrypt(&vault.vault_key, &new_key).await?);
        if vault.owned {
            owner_updates.push(VaultEnvelopeUpdate {
                vault_id: vault.vault_id,
                key_envelope,
            });
        } else {
            shared_updates.push(VaultShareEnvelopeUpdate {
                vault_id: vault.vault_id,
                user_id,
                key_envelope,
            });
        }
    }

    let totp_secret = rewrap_user_blob(
        crypto,
        repos.users.get_totp_secret(user_id).await?,
        BlobFormat::NonceCiphertext,
        old_key,
        &new_key,
    )
    .await?;

    let (recovery_phrase_envelope, recovery_key_envelope, recovery_key_available) = match recovery {
        RecoveryMaterial::LegacyPhraseEnvelope => {
            let rewrapped = rewrap_user_blob(
                crypto,
                repos.users.get_recovery_phrase_envelope(user_id).await?,
                BlobFormat::Versioned,
                old_key,
                &new_key,
            )
            .await?;
            (rewrapped, UserEnvelopeChange::Keep, true)
        }
        RecoveryMaterial::SealedAccountKey(given) => {
            let phrase = match given {
                Some(phrase) => Some(phrase.clone()),
                None => phrase_from_legacy_envelope(repos.users, crypto, user_id, old_key).await?,
            };
            let sealed = match phrase.as_ref() {
                Some(phrase) => UserEnvelopeChange::Replace(
                    seal_with_recovery_phrase(crypto, phrase, &new_key).await?,
                ),
                None => UserEnvelopeChange::Clear,
            };
            // The sealed account key replaces the stored phrase, which is no longer kept.
            (UserEnvelopeChange::Clear, sealed, phrase.is_some())
        }
    };

    let report = RekeyReport {
        owner_vaults_rewrapped: owner_updates.len(),
        shared_vaults_rewrapped: shared_updates.len(),
        vaults_without_key,
        secrets_readable,
        secrets_unreadable,
        totp_disabled: matches!(totp_secret, UserEnvelopeChange::Clear),
        recovery_key_available,
    };

    let password_envelope = encode_password_envelope(envelope);
    repos
        .vaults
        .apply_master_key_rotation_atomically(
            user_id,
            copy_secret(&password_envelope),
            owner_updates,
            shared_updates,
            UserKeyMaterialUpdate {
                recovery_phrase_envelope,
                totp_secret,
                recovery_key_envelope,
            },
        )
        .await?;

    verify_stored_envelope(repos.users, user_id, &password_envelope).await?;
    verify_vaults(&repos, crypto, user_id, &new_key, &vaults).await?;

    if report.totp_disabled {
        warn!(user_id = %user_id, "re-key: TOTP secret did not match the previous key; two-factor authentication disabled");
    }
    if !report.recovery_key_available {
        warn!(user_id = %user_id, "re-key: no readable recovery phrase; the recovery key must be sealed again");
    }

    Ok(RekeyOutcome {
        report,
        password_envelope,
        session_key: new_key,
    })
}

/// The recovery phrase a legacy account stored under its master key, when it still decrypts.
async fn phrase_from_legacy_envelope<U, C>(
    users: &U,
    crypto: &C,
    user_id: Uuid,
    legacy_key: &SecretBox<Vec<u8>>,
) -> Result<Option<SecretString>, AppError>
where
    U: UserRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    let Some(stored) = users.get_recovery_phrase_envelope(user_id).await? else {
        return Ok(None);
    };
    let Ok(payload) = decode_envelope(stored.expose_secret()) else {
        return Ok(None);
    };
    let Ok(plaintext) = crypto.decrypt(&payload, legacy_key).await else {
        return Ok(None);
    };
    Ok(std::str::from_utf8(plaintext.expose_secret())
        .ok()
        .map(|text| SecretString::new(text.into())))
}

async fn load_vault_envelope<U, V, E, S>(
    repos: &RekeyRepositories<'_, U, V, E, S>,
    vault_id: Uuid,
    owned: bool,
    user_id: Uuid,
) -> Result<Option<SecretBox<Vec<u8>>>, AppError>
where
    V: VaultRepository + Send + Sync,
    E: VaultKeyEnvelopeRepository + Send + Sync,
{
    if owned {
        repos.envelopes.get_vault_key_envelope(vault_id).await
    } else {
        repos.vaults.get_key_share(vault_id, user_id).await
    }
}

async fn unwrap_accessible_vaults<U, V, E, S, C>(
    repos: &RekeyRepositories<'_, U, V, E, S>,
    crypto: &C,
    user_id: Uuid,
    key: &SecretBox<Vec<u8>>,
) -> Result<(Vec<UnwrappedVault>, usize), AppError>
where
    V: VaultRepository + Send + Sync,
    E: VaultKeyEnvelopeRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    let mut vaults = Vec::new();
    let mut vaults_without_key = 0usize;

    for access in repos.vaults.get_accessible_vaults(user_id).await? {
        let vault_id = access.vault.id;
        let owned = access.vault.owner_user_id == user_id;

        let Some(envelope) = load_vault_envelope(repos, vault_id, owned, user_id).await? else {
            warn!(vault_id = %vault_id, "re-key: vault has no key envelope, left untouched");
            vaults_without_key = vaults_without_key.saturating_add(1);
            continue;
        };

        let vault_key = crypto
            .decrypt(&deserialize_vault_key_envelope(&envelope)?, key)
            .await
            .map_err(|_| {
                warn!(vault_id = %vault_id, "re-key aborted: vault does not open with the current account key");
                AppError::Recovery(RecoveryFailure::InconsistentKeyMaterial)
            })?;

        vaults.push(UnwrappedVault {
            vault_id,
            owned,
            vault_key,
        });
    }

    Ok((vaults, vaults_without_key))
}

/// Secrets are not rewritten by a re-key, so an unreadable one is reported, not fatal.
async fn check_secrets<S, C>(
    secrets: &S,
    crypto: &C,
    vaults: &[UnwrappedVault],
) -> Result<(usize, usize), AppError>
where
    S: SecretRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    let mut readable = 0usize;
    let mut unreadable = 0usize;

    for vault in vaults {
        for secret in secrets.list_by_vault_id(vault.vault_id).await? {
            let decrypts = match deserialize_vault_key_envelope(&secret.secret_blob) {
                Ok(payload) => crypto.decrypt(&payload, &vault.vault_key).await.is_ok(),
                Err(_) => false,
            };
            if decrypts {
                readable = readable.saturating_add(1);
            } else {
                unreadable = unreadable.saturating_add(1);
                warn!(secret_id = %secret.id, vault_id = %vault.vault_id, "re-key: secret does not decrypt with its vault key");
            }
        }
    }

    Ok((readable, unreadable))
}

/// A value that no longer decrypts with the old key could never be used again: it is cleared.
async fn rewrap_user_blob<C>(
    crypto: &C,
    stored: Option<SecretBox<Vec<u8>>>,
    format: BlobFormat,
    old_key: &SecretBox<Vec<u8>>,
    new_key: &SecretBox<Vec<u8>>,
) -> Result<UserEnvelopeChange, AppError>
where
    C: CryptoService + Send + Sync,
{
    let Some(stored) = stored else {
        return Ok(UserEnvelopeChange::Keep);
    };

    let plaintext = match format.decode(&stored) {
        Ok(payload) => crypto.decrypt(&payload, old_key).await.ok(),
        Err(_) => None,
    };
    let Some(plaintext) = plaintext else {
        return Ok(UserEnvelopeChange::Clear);
    };

    let payload = crypto.encrypt(&plaintext, new_key).await?;
    Ok(UserEnvelopeChange::Replace(format.encode(&payload)))
}

async fn verify_stored_envelope<U>(
    users: &U,
    user_id: Uuid,
    expected: &SecretBox<Vec<u8>>,
) -> Result<(), AppError>
where
    U: UserRepository + Send + Sync,
{
    let stored = users
        .get_password_envelope_by_user_id(user_id)
        .await?
        .ok_or_else(|| AppError::Storage("password envelope missing after re-key".to_string()))?;
    if bool::from(
        stored
            .expose_secret()
            .as_slice()
            .ct_eq(expected.expose_secret().as_slice()),
    ) {
        Ok(())
    } else {
        Err(AppError::Storage(
            "password envelope was not updated by the re-key".to_string(),
        ))
    }
}

async fn verify_vaults<U, V, E, S, C>(
    repos: &RekeyRepositories<'_, U, V, E, S>,
    crypto: &C,
    user_id: Uuid,
    new_key: &SecretBox<Vec<u8>>,
    vaults: &[UnwrappedVault],
) -> Result<(), AppError>
where
    V: VaultRepository + Send + Sync,
    E: VaultKeyEnvelopeRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    for vault in vaults {
        let envelope = load_vault_envelope(repos, vault.vault_id, vault.owned, user_id)
            .await?
            .ok_or_else(|| {
                AppError::Storage(format!(
                    "vault {} lost its key envelope during re-key",
                    vault.vault_id
                ))
            })?;
        let reopened = crypto
            .decrypt(&deserialize_vault_key_envelope(&envelope)?, new_key)
            .await
            .map_err(|_| {
                AppError::Crypto(format!(
                    "vault {} does not open with the new key after re-key",
                    vault.vault_id
                ))
            })?;
        if !bool::from(
            reopened
                .expose_secret()
                .as_slice()
                .ct_eq(vault.vault_key.expose_secret().as_slice()),
        ) {
            return Err(AppError::Crypto(format!(
                "vault {} key changed during re-key",
                vault.vault_id
            )));
        }
    }

    Ok(())
}

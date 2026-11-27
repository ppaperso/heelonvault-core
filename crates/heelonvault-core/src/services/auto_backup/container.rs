//! `.hvba` container: an automatic backup sealed with the account key.
//!
//! The manual `.hvb` export is sealed with the recovery phrase, which the application never
//! stores, so it cannot run unattended. An automatic backup is instead encrypted with a random
//! data key, wrapped with the account key that is unlocked during the session. The header
//! carries the account-key envelopes already stored in the database (under the master password
//! and under the recovery phrase, both ciphertexts), so a backup opens with either credential
//! and nothing new is stored anywhere.
//!
//! Layout: `magic || version (u16 LE) || header_len (u32 LE) || header JSON || ciphertext`.
//! The whole prefix is the payload AAD: altering any header field fails decryption.

use aes_gcm::aead::consts::U12;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use secrecy::{ExposeSecret, SecretBox, SecretString};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::errors::{AppError, RecoveryFailure};
use crate::services::account_key::{ACCOUNT_KEY_LEN, open_with_recovery_phrase};
use crate::services::auth_service::{
    PasswordEnvelope, decode_password_envelope, unlock_password_envelope,
};
use crate::services::crypto_service::CryptoService;

pub(crate) const MAGIC: &[u8; 4] = b"HVBA";
const FORMAT_VERSION: u16 = 1;
const PREFIX_LEN: usize = MAGIC.len() + 2 + 4;
const MAX_HEADER_LEN: u32 = 16 * 1024;
const NONCE_LEN: usize = 12;
const DATA_KEY_LEN: usize = 32;
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
/// AAD of the data-key wrap: keeps it apart from every other use of the account key.
const WRAP_DOMAIN: &[u8] = b"heelonvault/auto-backup/data-key/v1\n";

#[derive(Debug, Serialize, Deserialize)]
struct Header {
    user_id: Uuid,
    created_at: String,
    app_version: String,
    password_envelope_b64: String,
    #[serde(default)]
    recovery_envelope_b64: Option<String>,
    wrap_nonce_b64: String,
    wrapped_key_b64: String,
    payload_nonce_b64: String,
    plaintext_size: u64,
}

/// Account-key envelopes copied into each backup, as stored in `users`.
#[derive(Debug)]
#[non_exhaustive]
pub struct AccountEnvelopes {
    pub user_id: Uuid,
    /// `users.password_envelope`; must be an account-key envelope (not a legacy one).
    pub password_envelope: SecretBox<Vec<u8>>,
    /// Account key sealed with the recovery phrase, when the account has one.
    pub recovery_envelope: Option<SecretBox<Vec<u8>>>,
}

impl AccountEnvelopes {
    pub fn new(
        user_id: Uuid,
        password_envelope: SecretBox<Vec<u8>>,
        recovery_envelope: Option<SecretBox<Vec<u8>>>,
    ) -> Self {
        Self {
            user_id,
            password_envelope,
            recovery_envelope,
        }
    }
}

/// Public, unauthenticated description of a backup, read without any key. It is checked
/// again by the AAD when the backup is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackupInfo {
    pub user_id: Uuid,
    pub created_at: OffsetDateTime,
    pub app_version: String,
    pub plaintext_size: u64,
    /// The backup also opens with the recovery phrase.
    pub opens_with_recovery_phrase: bool,
}

/// Credential that unlocks the account key stored in a backup.
#[non_exhaustive]
pub enum BackupCredential {
    MasterPassword(SecretString),
    RecoveryPhrase(SecretString),
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

fn random_bytes<const N: usize>() -> Result<Zeroizing<[u8; N]>, AppError> {
    let mut bytes = Zeroizing::new([0_u8; N]);
    getrandom::fill(bytes.as_mut_slice())
        .map_err(|err| AppError::Crypto(format!("random generation failed: {err}")))?;
    Ok(bytes)
}

fn cipher(key: &[u8]) -> Result<Aes256Gcm, AppError> {
    if key.len() != ACCOUNT_KEY_LEN {
        return Err(AppError::Crypto("unexpected key length".to_string()));
    }
    Aes256Gcm::new_from_slice(key).map_err(|err| AppError::Crypto(format!("invalid key: {err}")))
}

fn wrap_aad(user_id: Uuid) -> Vec<u8> {
    let mut aad = Vec::with_capacity(WRAP_DOMAIN.len() + 16);
    aad.extend_from_slice(WRAP_DOMAIN);
    aad.extend_from_slice(user_id.as_bytes());
    aad
}

fn decode_nonce(value: &str) -> Result<[u8; NONCE_LEN], AppError> {
    let bytes = b64()
        .decode(value.as_bytes())
        .map_err(|_| AppError::Validation("invalid backup nonce".to_string()))?;
    bytes
        .try_into()
        .map_err(|_| AppError::Validation("backup nonce has an invalid length".to_string()))
}

/// The bytes start like an automatic backup (and not like a manual `.hvb` export).
pub fn is_auto_backup(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// Encrypts a SQLite snapshot into an `.hvba` container.
pub fn seal(
    sqlite_bytes: &[u8],
    account_key: &SecretBox<Vec<u8>>,
    envelopes: &AccountEnvelopes,
    created_at: OffsetDateTime,
) -> Result<Vec<u8>, AppError> {
    if !sqlite_bytes.starts_with(SQLITE_HEADER) {
        return Err(AppError::Validation(
            "the snapshot is not a SQLite database".to_string(),
        ));
    }
    // A legacy envelope stores the derived key itself: copying it next to the data would
    // let anyone open the backup.
    match decode_password_envelope(&envelopes.password_envelope)? {
        PasswordEnvelope::AccountKey { .. } => {}
        PasswordEnvelope::Legacy { .. } => {
            return Err(AppError::Validation(
                "automatic backups need an account-key envelope".to_string(),
            ));
        }
    }

    let data_key = random_bytes::<DATA_KEY_LEN>()?;
    let wrap_nonce = random_bytes::<NONCE_LEN>()?;
    let payload_nonce = random_bytes::<NONCE_LEN>()?;

    let wrap_nonce_ga: Nonce<U12> = (*wrap_nonce).into();
    let wrapped_key = cipher(account_key.expose_secret())?
        .encrypt(
            &wrap_nonce_ga,
            Payload {
                msg: data_key.as_slice(),
                aad: wrap_aad(envelopes.user_id).as_slice(),
            },
        )
        .map_err(|_| AppError::Crypto("backup key wrap failed".to_string()))?;

    let header = Header {
        user_id: envelopes.user_id,
        created_at: created_at
            .format(&Rfc3339)
            .map_err(|_| AppError::Validation("invalid backup timestamp".to_string()))?,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        password_envelope_b64: b64().encode(envelopes.password_envelope.expose_secret()),
        recovery_envelope_b64: envelopes
            .recovery_envelope
            .as_ref()
            .map(|envelope| b64().encode(envelope.expose_secret())),
        wrap_nonce_b64: b64().encode(wrap_nonce.as_slice()),
        wrapped_key_b64: b64().encode(wrapped_key),
        payload_nonce_b64: b64().encode(payload_nonce.as_slice()),
        plaintext_size: sqlite_bytes.len() as u64,
    };
    let header_bytes = serde_json::to_vec(&header)
        .map_err(|err| AppError::Storage(format!("failed to serialize backup header: {err}")))?;
    let header_len = u32::try_from(header_bytes.len())
        .ok()
        .filter(|len| *len <= MAX_HEADER_LEN)
        .ok_or_else(|| AppError::Validation("backup header is too large".to_string()))?;

    let mut container = Vec::with_capacity(PREFIX_LEN + header_bytes.len() + sqlite_bytes.len());
    container.extend_from_slice(MAGIC);
    container.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    container.extend_from_slice(&header_len.to_le_bytes());
    container.extend_from_slice(&header_bytes);

    let payload_nonce_ga: Nonce<U12> = (*payload_nonce).into();
    let ciphertext = cipher(data_key.as_slice())?
        .encrypt(
            &payload_nonce_ga,
            Payload {
                msg: sqlite_bytes,
                aad: container.as_slice(),
            },
        )
        .map_err(|_| AppError::Crypto("backup encryption failed".to_string()))?;
    container.extend_from_slice(&ciphertext);
    Ok(container)
}

/// Splits a container into its authenticated prefix, header and ciphertext.
fn split(bytes: &[u8]) -> Result<(&[u8], Header, &[u8]), AppError> {
    if bytes.len() < PREFIX_LEN || &bytes[..MAGIC.len()] != MAGIC {
        return Err(AppError::Recovery(RecoveryFailure::NotABackup));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != FORMAT_VERSION {
        return Err(AppError::Recovery(
            RecoveryFailure::UnsupportedBackupVersion(version),
        ));
    }
    let header_len = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    if header_len == 0 || header_len > MAX_HEADER_LEN {
        return Err(AppError::Validation(
            "backup header length is out of range".to_string(),
        ));
    }
    let header_end = PREFIX_LEN + header_len as usize;
    if bytes.len() <= header_end {
        return Err(AppError::Validation("backup file is truncated".to_string()));
    }
    let header: Header = serde_json::from_slice(&bytes[PREFIX_LEN..header_end])
        .map_err(|_| AppError::Validation("invalid backup header".to_string()))?;
    Ok((&bytes[..header_end], header, &bytes[header_end..]))
}

/// Reads the public description of a backup without opening it.
pub fn read_info(bytes: &[u8]) -> Result<BackupInfo, AppError> {
    let (_, header, _) = split(bytes)?;
    Ok(BackupInfo {
        user_id: header.user_id,
        created_at: OffsetDateTime::parse(&header.created_at, &Rfc3339)
            .map_err(|_| AppError::Validation("invalid backup timestamp".to_string()))?,
        app_version: header.app_version,
        plaintext_size: header.plaintext_size,
        opens_with_recovery_phrase: header.recovery_envelope_b64.is_some(),
    })
}

/// Decrypts a container with the account key and returns the SQLite database.
pub fn open_with_account_key(
    bytes: &[u8],
    account_key: &SecretBox<Vec<u8>>,
) -> Result<Zeroizing<Vec<u8>>, AppError> {
    let (prefix, header, ciphertext) = split(bytes)?;
    let wrapped_key = b64()
        .decode(header.wrapped_key_b64.as_bytes())
        .map_err(|_| AppError::Validation("invalid wrapped backup key".to_string()))?;
    let wrap_nonce: Nonce<U12> = decode_nonce(&header.wrap_nonce_b64)?.into();
    let data_key = Zeroizing::new(
        cipher(account_key.expose_secret())?
            .decrypt(
                &wrap_nonce,
                Payload {
                    msg: wrapped_key.as_slice(),
                    aad: wrap_aad(header.user_id).as_slice(),
                },
            )
            .map_err(|_| AppError::Recovery(RecoveryFailure::WrongPhraseOrAlteredFile))?,
    );
    let payload_nonce: Nonce<U12> = decode_nonce(&header.payload_nonce_b64)?.into();
    let plaintext = Zeroizing::new(
        cipher(data_key.as_slice())?
            .decrypt(
                &payload_nonce,
                Payload {
                    msg: ciphertext,
                    aad: prefix,
                },
            )
            .map_err(|_| AppError::Recovery(RecoveryFailure::WrongPhraseOrAlteredFile))?,
    );
    if !plaintext.starts_with(SQLITE_HEADER) || plaintext.len() as u64 != header.plaintext_size {
        return Err(AppError::Validation(
            "backup content does not match its header".to_string(),
        ));
    }
    Ok(plaintext)
}

/// Recovers the account key from the envelopes stored in the backup header.
pub async fn unlock_account_key<C>(
    bytes: &[u8],
    crypto: &C,
    credential: &BackupCredential,
) -> Result<SecretBox<Vec<u8>>, AppError>
where
    C: CryptoService + Send + Sync,
{
    let (_, header, _) = split(bytes)?;
    match credential {
        BackupCredential::MasterPassword(password) => {
            let envelope = b64()
                .decode(header.password_envelope_b64.as_bytes())
                .map_err(|_| AppError::Validation("invalid password envelope".to_string()))?;
            let envelope = decode_password_envelope(&SecretBox::new(Box::new(envelope)))?;
            if !envelope.is_account_key() {
                return Err(AppError::Validation(
                    "backup password envelope is not an account-key envelope".to_string(),
                ));
            }
            unlock_password_envelope(crypto, &envelope, password)
                .await?
                .ok_or(AppError::Recovery(
                    RecoveryFailure::WrongPhraseOrAlteredFile,
                ))
        }
        BackupCredential::RecoveryPhrase(phrase) => {
            let encoded = header
                .recovery_envelope_b64
                .as_ref()
                .ok_or(AppError::Recovery(RecoveryFailure::RecoveryKeyMissing))?;
            let envelope = b64()
                .decode(encoded.as_bytes())
                .map_err(|_| AppError::Validation("invalid recovery envelope".to_string()))?;
            open_with_recovery_phrase(crypto, phrase, &SecretBox::new(Box::new(envelope)))
                .await
                .map_err(|_| AppError::Recovery(RecoveryFailure::WrongPhraseOrAlteredFile))
        }
    }
}

/// Opens a backup with the master password or the recovery phrase.
pub async fn open_with_credential<C>(
    bytes: &[u8],
    crypto: &C,
    credential: &BackupCredential,
) -> Result<Zeroizing<Vec<u8>>, AppError>
where
    C: CryptoService + Send + Sync,
{
    let account_key = unlock_account_key(bytes, crypto, credential).await?;
    open_with_account_key(bytes, &account_key)
}

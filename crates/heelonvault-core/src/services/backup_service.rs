use std::fs;
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::errors::AppError;
use aes_gcm::aead::Aead;
use aes_gcm::aead::Payload;
use aes_gcm::aead::consts::U12;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use bip39::{Language, Mnemonic};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tracing::info;
use zeroize::Zeroizing;

const BACKUP_NONCE_LEN: usize = 12;
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
const AES_256_KEY_LEN: usize = 32;
const RECOVERY_SALT_LEN: usize = 32;
const RECOVERY_VERIFIER_VERSION: u8 = 1;
const RECOVERY_VERIFIER_TAG_LEN: usize = 32;
const RECOVERY_VERIFIER_LEN: usize = 1 + RECOVERY_SALT_LEN + RECOVERY_VERIFIER_TAG_LEN;
const RECOVERY_VERIFIER_DOMAIN: &[u8] = b"heelonvault/recovery-verifier/v1";

/// `.hvb` container: magic || version(u16 LE) || header_len(u32 LE) || header JSON || ciphertext.
/// The whole prefix is fed to AES-GCM as AAD, so KDF parameters and salt are authenticated.
const HVB_MAGIC: &[u8; 4] = b"HVBK";
const HVB_FORMAT_VERSION: u16 = 2;
const HVB_PREFIX_LEN: usize = HVB_MAGIC.len() + 2 + 4;
const HVB_MAX_HEADER_LEN: u32 = 8 * 1024;
const HVB_KDF_NAME: &str = "argon2id";
const HVB_DEFAULT_M_COST_KIB: u32 = 64 * 1024;
const HVB_DEFAULT_T_COST: u32 = 3;
const HVB_DEFAULT_P_COST: u32 = 1;
/// Bounds refuse both a weakened KDF forged by an attacker and a memory-bomb header.
const HVB_MIN_M_COST_KIB: u32 = 19 * 1024;
const HVB_MAX_M_COST_KIB: u32 = 1024 * 1024;
const HVB_MIN_T_COST: u32 = 2;
const HVB_MAX_T_COST: u32 = 16;
const HVB_MAX_P_COST: u32 = 16;

#[derive(Debug, Clone)]
pub struct RecoveryKeyBundle {
    pub recovery_phrase: SecretString,
}

#[derive(Debug, Serialize, Deserialize)]
struct HvbHeaderV2 {
    kdf: String,
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
    salt_b64: String,
    nonce_b64: String,
    plaintext_size: u64,
}

#[derive(Debug, Clone)]
pub struct BackupMetadata {
    /// Digest of the encrypted container, usable as a file identifier.
    /// Never the digest of the plaintext: that would confirm vault contents to anyone
    /// holding the file.
    pub sha256_hex: String,
    pub plaintext_size: usize,
}

#[trait_variant::make(BackupService: Send)]
pub trait LocalBackupService {
    fn generate_recovery_key(&self) -> Result<RecoveryKeyBundle, AppError>;
    /// Build the at-rest verifier for a recovery phrase. Stored instead of the phrase
    /// itself so an export can only proceed with a phrase the user actually re-types.
    fn build_recovery_verifier(
        &self,
        recovery_phrase: &SecretString,
    ) -> Result<SecretBox<Vec<u8>>, AppError>;
    /// Constant-time check of a re-typed phrase against a stored verifier.
    fn verify_recovery_phrase(
        &self,
        recovery_phrase: &SecretString,
        verifier: &[u8],
    ) -> Result<bool, AppError>;
    fn export_hvb_with_recovery_key(
        &self,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
    ) -> Result<BackupMetadata, AppError>;
    fn import_hvb_with_recovery_key(
        &self,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
        new_sqlite_db_path: &Path,
    ) -> Result<BackupMetadata, AppError>;
}

pub struct BackupServiceImpl;

impl BackupServiceImpl {
    pub fn new() -> Self {
        Self
    }

    fn validate_sqlite_bytes(bytes: &[u8]) -> Result<(), AppError> {
        if bytes.len() < SQLITE_HEADER.len() || !bytes.starts_with(SQLITE_HEADER) {
            return Err(AppError::Validation(
                "input is not a valid SQLite file header".to_string(),
            ));
        }

        Ok(())
    }

    fn sha256_hex(bytes: &[u8]) -> Result<String, AppError> {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    fn generate_nonce() -> Result<[u8; BACKUP_NONCE_LEN], AppError> {
        let mut nonce = [0_u8; BACKUP_NONCE_LEN];
        getrandom::fill(&mut nonce)
            .map_err(|err| AppError::Crypto(format!("backup nonce generation failed: {err}")))?;
        Ok(nonce)
    }

    fn generate_recovery_salt() -> Result<[u8; RECOVERY_SALT_LEN], AppError> {
        let mut salt = [0_u8; RECOVERY_SALT_LEN];
        getrandom::fill(&mut salt)
            .map_err(|err| AppError::Crypto(format!("recovery salt generation failed: {err}")))?;
        Ok(salt)
    }

    fn derive_backup_key_from_recovery(
        recovery_phrase: &SecretString,
        salt: &[u8],
        m_cost_kib: u32,
        t_cost: u32,
        p_cost: u32,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        if salt.is_empty() {
            return Err(AppError::Validation(
                "recovery salt must not be empty".to_string(),
            ));
        }

        let params = Params::new(m_cost_kib, t_cost, p_cost, Some(AES_256_KEY_LEN))
            .map_err(|err| AppError::Crypto(format!("invalid argon2 params: {err}")))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut output = Zeroizing::new(vec![0_u8; AES_256_KEY_LEN]);
        argon2
            .hash_password_into(
                recovery_phrase.expose_secret().as_bytes(),
                salt,
                output.as_mut_slice(),
            )
            .map_err(|err| {
                AppError::Crypto(format!("argon2id recovery derivation failed: {err}"))
            })?;

        Ok(SecretBox::new(Box::new(output.to_vec())))
    }

    /// Domain-separated from the backup key derivation so a stored verifier can never
    /// be replayed as the key that decrypts a `.hvb`.
    fn derive_recovery_verifier_tag(
        recovery_phrase: &SecretString,
        salt: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, AppError> {
        if salt.len() != RECOVERY_SALT_LEN {
            return Err(AppError::Validation(
                "recovery verifier salt has an invalid length".to_string(),
            ));
        }

        let params = Params::new(64 * 1024, 3, 1, Some(RECOVERY_VERIFIER_TAG_LEN))
            .map_err(|err| AppError::Crypto(format!("invalid argon2 params: {err}")))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut input = Zeroizing::new(Vec::with_capacity(
            RECOVERY_VERIFIER_DOMAIN.len() + recovery_phrase.expose_secret().len(),
        ));
        input.extend_from_slice(RECOVERY_VERIFIER_DOMAIN);
        input.extend_from_slice(recovery_phrase.expose_secret().as_bytes());

        let mut tag = Zeroizing::new(vec![0_u8; RECOVERY_VERIFIER_TAG_LEN]);
        argon2
            .hash_password_into(input.as_slice(), salt, tag.as_mut_slice())
            .map_err(|err| {
                AppError::Crypto(format!("argon2id verifier derivation failed: {err}"))
            })?;

        Ok(tag)
    }

    /// Serializes the authenticated prefix. Returned separately from the ciphertext so the
    /// exact same bytes are used as AAD on both encrypt and decrypt.
    fn build_hvb_prefix(header_bytes: &[u8]) -> Result<Vec<u8>, AppError> {
        let header_len = u32::try_from(header_bytes.len()).map_err(|_| {
            AppError::Validation("`.hvb` header is too large to encode".to_string())
        })?;
        if header_len > HVB_MAX_HEADER_LEN {
            return Err(AppError::Validation(
                "`.hvb` header exceeds the maximum allowed size".to_string(),
            ));
        }

        let mut prefix = Vec::with_capacity(HVB_PREFIX_LEN + header_bytes.len());
        prefix.extend_from_slice(HVB_MAGIC);
        prefix.extend_from_slice(&HVB_FORMAT_VERSION.to_le_bytes());
        prefix.extend_from_slice(&header_len.to_le_bytes());
        prefix.extend_from_slice(header_bytes);
        Ok(prefix)
    }

    /// Splits a `.hvb` file into its authenticated prefix, decoded header and ciphertext.
    fn split_hvb_container(bytes: &[u8]) -> Result<(Vec<u8>, HvbHeaderV2, &[u8]), AppError> {
        if bytes.len() < HVB_PREFIX_LEN {
            return Err(AppError::Validation(
                "not a .hvb backup file: too short".to_string(),
            ));
        }
        if &bytes[..HVB_MAGIC.len()] != HVB_MAGIC {
            return Err(AppError::Validation(
                "not a .hvb backup file: bad magic".to_string(),
            ));
        }

        let mut version_bytes = [0_u8; 2];
        version_bytes.copy_from_slice(&bytes[HVB_MAGIC.len()..HVB_MAGIC.len() + 2]);
        let version = u16::from_le_bytes(version_bytes);
        if version != HVB_FORMAT_VERSION {
            return Err(AppError::Validation(format!(
                "unsupported .hvb format version {version}; this build reads version {HVB_FORMAT_VERSION} only"
            )));
        }

        let mut len_bytes = [0_u8; 4];
        len_bytes.copy_from_slice(&bytes[HVB_MAGIC.len() + 2..HVB_PREFIX_LEN]);
        let header_len = u32::from_le_bytes(len_bytes);
        if header_len == 0 || header_len > HVB_MAX_HEADER_LEN {
            return Err(AppError::Validation(
                "`.hvb` header length is out of range".to_string(),
            ));
        }

        let header_end = HVB_PREFIX_LEN
            .checked_add(header_len as usize)
            .ok_or_else(|| AppError::Validation("`.hvb` header length overflows".to_string()))?;
        if bytes.len() <= header_end {
            return Err(AppError::Validation(
                "`.hvb` file is truncated: no ciphertext".to_string(),
            ));
        }

        let header: HvbHeaderV2 = serde_json::from_slice(&bytes[HVB_PREFIX_LEN..header_end])
            .map_err(|err| AppError::Validation(format!("invalid .hvb header: {err}")))?;

        Ok((bytes[..header_end].to_vec(), header, &bytes[header_end..]))
    }

    fn validate_kdf_params(header: &HvbHeaderV2) -> Result<(), AppError> {
        if header.kdf != HVB_KDF_NAME {
            return Err(AppError::Validation(format!(
                "unsupported .hvb kdf: {}",
                header.kdf
            )));
        }
        if !(HVB_MIN_M_COST_KIB..=HVB_MAX_M_COST_KIB).contains(&header.m_cost_kib) {
            return Err(AppError::Validation(
                "`.hvb` argon2 memory cost is out of the accepted range".to_string(),
            ));
        }
        if !(HVB_MIN_T_COST..=HVB_MAX_T_COST).contains(&header.t_cost) {
            return Err(AppError::Validation(
                "`.hvb` argon2 time cost is out of the accepted range".to_string(),
            ));
        }
        if header.p_cost == 0 || header.p_cost > HVB_MAX_P_COST {
            return Err(AppError::Validation(
                "`.hvb` argon2 parallelism is out of the accepted range".to_string(),
            ));
        }
        Ok(())
    }

    /// Writes through a temporary sibling then renames, so a failed export never leaves a
    /// truncated `.hvb` behind at the destination.
    fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
        Self::ensure_parent_exists(path)?;

        let mut temp_path = path.to_path_buf();
        let temp_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => format!(".{name}.partial"),
            None => ".hvb.partial".to_string(),
        };
        temp_path.set_file_name(temp_name);

        if temp_path.exists() {
            fs::remove_file(&temp_path).map_err(AppError::Io)?;
        }

        let write_result = (|| -> Result<(), AppError> {
            let mut file = fs::File::create(&temp_path).map_err(AppError::Io)?;
            Self::set_owner_only_file_permissions(&temp_path)?;
            file.write_all(bytes).map_err(AppError::Io)?;
            file.sync_all().map_err(AppError::Io)?;
            Ok(())
        })();

        if let Err(err) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }

        if let Err(err) = fs::rename(&temp_path, path).map_err(AppError::Io) {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }

        Self::set_owner_only_file_permissions(path)
    }

    /// Single decryption path for `.hvb` v2: validates the container, authenticates the
    /// header through AAD and returns the SQLite plaintext.
    fn open_hvb_container(
        bytes: &[u8],
        recovery_phrase: &SecretString,
    ) -> Result<Vec<u8>, AppError> {
        let (prefix, header, ciphertext) = Self::split_hvb_container(bytes)?;
        Self::validate_kdf_params(&header)?;

        let salt = base64::engine::general_purpose::STANDARD
            .decode(header.salt_b64.as_bytes())
            .map_err(|err| AppError::Validation(format!("invalid .hvb salt: {err}")))?;
        if salt.len() != RECOVERY_SALT_LEN {
            return Err(AppError::Validation(
                "`.hvb` salt has an invalid length".to_string(),
            ));
        }

        let nonce_vec = base64::engine::general_purpose::STANDARD
            .decode(header.nonce_b64.as_bytes())
            .map_err(|err| AppError::Validation(format!("invalid .hvb nonce: {err}")))?;
        if nonce_vec.len() != BACKUP_NONCE_LEN {
            return Err(AppError::Validation(
                "`.hvb` nonce has an invalid length".to_string(),
            ));
        }
        let mut nonce = [0_u8; BACKUP_NONCE_LEN];
        nonce.copy_from_slice(nonce_vec.as_slice());

        let backup_key = Self::derive_backup_key_from_recovery(
            recovery_phrase,
            salt.as_slice(),
            header.m_cost_kib,
            header.t_cost,
            header.p_cost,
        )?;

        let cipher = Aes256Gcm::new_from_slice(backup_key.expose_secret().as_slice())
            .map_err(|err| AppError::Crypto(format!("invalid backup key: {err}")))?;
        let nonce_ga: Nonce<U12> = nonce.into();
        let plaintext = cipher
            .decrypt(
                &nonce_ga,
                Payload {
                    msg: ciphertext,
                    aad: prefix.as_slice(),
                },
            )
            .map_err(|_| {
                AppError::Crypto(
                    "backup decryption failed: wrong recovery key or altered file".to_string(),
                )
            })?;

        Self::validate_sqlite_bytes(plaintext.as_slice())?;
        if plaintext.len() as u64 != header.plaintext_size {
            return Err(AppError::Validation(
                "`.hvb` plaintext size does not match its header".to_string(),
            ));
        }

        Ok(plaintext)
    }

    fn replace_existing_database(
        target_sqlite_db_path: &Path,
        plaintext: &[u8],
    ) -> Result<(), AppError> {
        Self::ensure_parent_exists(target_sqlite_db_path)?;

        if target_sqlite_db_path.exists() {
            let old_path = target_sqlite_db_path.with_extension("old");
            if old_path.exists() {
                fs::remove_file(&old_path).map_err(AppError::Io)?;
            }

            fs::rename(target_sqlite_db_path, &old_path).map_err(AppError::Io)?;
        }

        fs::write(target_sqlite_db_path, plaintext).map_err(AppError::Io)?;
        Self::set_owner_only_file_permissions(target_sqlite_db_path)?;
        Ok(())
    }

    fn ensure_parent_exists(path: &Path) -> Result<(), AppError> {
        match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                fs::create_dir_all(parent).map_err(AppError::Io)
            }
            _ => Ok(()),
        }
    }

    fn set_owner_only_file_permissions(_path: &Path) -> Result<(), AppError> {
        #[cfg(unix)]
        {
            fs::set_permissions(_path, fs::Permissions::from_mode(0o600)).map_err(AppError::Io)?;
        }
        Ok(())
    }
}

impl Default for BackupServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl BackupService for BackupServiceImpl {
    fn generate_recovery_key(&self) -> Result<RecoveryKeyBundle, AppError> {
        let mut entropy = [0_u8; 32];
        getrandom::fill(&mut entropy)
            .map_err(|err| AppError::Crypto(format!("failed to gather recovery entropy: {err}")))?;

        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .map_err(|err| AppError::Crypto(format!("failed to generate bip39 mnemonic: {err}")))?;

        Ok(RecoveryKeyBundle {
            recovery_phrase: SecretString::new(mnemonic.to_string().into_boxed_str()),
        })
    }

    fn build_recovery_verifier(
        &self,
        recovery_phrase: &SecretString,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        let salt = Self::generate_recovery_salt()?;
        let tag = Self::derive_recovery_verifier_tag(recovery_phrase, &salt)?;

        let mut verifier = Vec::with_capacity(RECOVERY_VERIFIER_LEN);
        verifier.push(RECOVERY_VERIFIER_VERSION);
        verifier.extend_from_slice(&salt);
        verifier.extend_from_slice(tag.as_slice());
        Ok(SecretBox::new(Box::new(verifier)))
    }

    fn verify_recovery_phrase(
        &self,
        recovery_phrase: &SecretString,
        verifier: &[u8],
    ) -> Result<bool, AppError> {
        if verifier.len() != RECOVERY_VERIFIER_LEN {
            return Err(AppError::Validation(
                "stored recovery verifier has an invalid length".to_string(),
            ));
        }
        if verifier[0] != RECOVERY_VERIFIER_VERSION {
            return Err(AppError::Validation(format!(
                "unsupported recovery verifier version: {}",
                verifier[0]
            )));
        }

        let salt = &verifier[1..=RECOVERY_SALT_LEN];
        let expected = &verifier[1 + RECOVERY_SALT_LEN..];
        let candidate = Self::derive_recovery_verifier_tag(recovery_phrase, salt)?;

        Ok(bool::from(candidate.as_slice().ct_eq(expected)))
    }

    fn export_hvb_with_recovery_key(
        &self,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
    ) -> Result<BackupMetadata, AppError> {
        let sqlite_bytes = Zeroizing::new(fs::read(sqlite_db_path).map_err(AppError::Io)?);
        Self::validate_sqlite_bytes(sqlite_bytes.as_slice())?;

        let salt = Self::generate_recovery_salt()?;
        let nonce = Self::generate_nonce()?;
        let backup_key = Self::derive_backup_key_from_recovery(
            recovery_phrase,
            &salt,
            HVB_DEFAULT_M_COST_KIB,
            HVB_DEFAULT_T_COST,
            HVB_DEFAULT_P_COST,
        )?;

        let header = HvbHeaderV2 {
            kdf: HVB_KDF_NAME.to_string(),
            m_cost_kib: HVB_DEFAULT_M_COST_KIB,
            t_cost: HVB_DEFAULT_T_COST,
            p_cost: HVB_DEFAULT_P_COST,
            salt_b64: base64::engine::general_purpose::STANDARD.encode(salt),
            nonce_b64: base64::engine::general_purpose::STANDARD.encode(nonce),
            plaintext_size: sqlite_bytes.len() as u64,
        };
        let header_bytes = serde_json::to_vec(&header)
            .map_err(|err| AppError::Storage(format!("failed to serialize hvb header: {err}")))?;
        let prefix = Self::build_hvb_prefix(header_bytes.as_slice())?;

        let cipher = Aes256Gcm::new_from_slice(backup_key.expose_secret().as_slice())
            .map_err(|err| AppError::Crypto(format!("invalid backup key: {err}")))?;
        let nonce_ga: Nonce<U12> = nonce.into();
        let ciphertext = cipher
            .encrypt(
                &nonce_ga,
                Payload {
                    msg: sqlite_bytes.as_slice(),
                    aad: prefix.as_slice(),
                },
            )
            .map_err(|_| AppError::Crypto("backup encryption failed".to_string()))?;

        let mut container = Vec::with_capacity(prefix.len() + ciphertext.len());
        container.extend_from_slice(prefix.as_slice());
        container.extend_from_slice(ciphertext.as_slice());

        Self::write_atomic(backup_file_path, container.as_slice())?;

        // Re-read and decrypt what actually landed on disk before reporting success.
        let written = fs::read(backup_file_path).map_err(AppError::Io)?;
        let restored = Zeroizing::new(Self::open_hvb_container(
            written.as_slice(),
            recovery_phrase,
        )?);
        if restored.as_slice() != sqlite_bytes.as_slice() {
            let _ = fs::remove_file(backup_file_path);
            return Err(AppError::Validation(
                "written .hvb self-check failed: the file does not decrypt to the source database"
                    .to_string(),
            ));
        }

        let sha256_hex = Self::sha256_hex(written.as_slice())?;
        info!(
            file = %backup_file_path.display(),
            plaintext_size = sqlite_bytes.len(),
            "encrypted hvb export completed successfully"
        );

        Ok(BackupMetadata {
            sha256_hex,
            plaintext_size: sqlite_bytes.len(),
        })
    }

    fn import_hvb_with_recovery_key(
        &self,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
        new_sqlite_db_path: &Path,
    ) -> Result<BackupMetadata, AppError> {
        // Catches a mistyped word before spending seconds in Argon2.
        Mnemonic::parse_in_normalized(Language::English, recovery_phrase.expose_secret())
            .map_err(|err| AppError::Validation(format!("invalid recovery phrase: {err}")))?;

        let backup_bytes = fs::read(backup_file_path).map_err(AppError::Io)?;
        let plaintext = Zeroizing::new(Self::open_hvb_container(
            backup_bytes.as_slice(),
            recovery_phrase,
        )?);

        Self::replace_existing_database(new_sqlite_db_path, plaintext.as_slice())?;

        let sha256_hex = Self::sha256_hex(backup_bytes.as_slice())?;
        info!(
            file = %backup_file_path.display(),
            plaintext_size = plaintext.len(),
            "encrypted hvb import completed successfully"
        );

        Ok(BackupMetadata {
            sha256_hex,
            plaintext_size: plaintext.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use secrecy::ExposeSecret;
    use uuid::Uuid;

    use crate::errors::AppError;

    use super::{
        BackupService, BackupServiceImpl, HVB_DEFAULT_M_COST_KIB, HVB_DEFAULT_P_COST,
        HVB_DEFAULT_T_COST, HVB_KDF_NAME, HVB_PREFIX_LEN, HvbHeaderV2, RECOVERY_SALT_LEN,
        RECOVERY_VERIFIER_TAG_LEN,
    };

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new() -> Result<Self, AppError> {
            let path =
                std::env::temp_dir().join(format!("heelonvault-backup-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).map_err(AppError::Io)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn sample_sqlite_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"SQLite format 3\0");
        bytes.extend_from_slice(&[0_u8; 256]);
        bytes
    }

    fn write_sample_sqlite(path: &Path) -> Result<Vec<u8>, AppError> {
        let bytes = sample_sqlite_bytes();
        fs::write(path, bytes.as_slice()).map_err(AppError::Io)?;
        Ok(bytes)
    }

    /// The phrase shown at bootstrap must be the one that decrypts a later export.
    /// Regression guard for the divergence between the displayed and the stored key.
    #[test]
    fn export_then_import_with_same_recovery_phrase_restores_database() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let source_db_path = temp_dir.path().join("source.db");
        let backup_path = temp_dir.path().join("backup.hvb");
        let restored_db_path = temp_dir.path().join("restored.db");
        let original_bytes_result = write_sample_sqlite(&source_db_path);
        assert!(original_bytes_result.is_ok(), "sqlite seed should succeed");
        let original_bytes = match original_bytes_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let service = BackupServiceImpl::new();
        let bundle_result = service.generate_recovery_key();
        assert!(bundle_result.is_ok(), "recovery key generation should work");
        let bundle = match bundle_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let export_result = service.export_hvb_with_recovery_key(
            &source_db_path,
            &backup_path,
            &bundle.recovery_phrase,
        );
        assert!(export_result.is_ok(), "export should succeed");

        let import_result = service.import_hvb_with_recovery_key(
            &backup_path,
            &bundle.recovery_phrase,
            &restored_db_path,
        );
        assert!(
            import_result.is_ok(),
            "the phrase handed to the user must restore the backup"
        );

        let restored = fs::read(&restored_db_path);
        assert!(restored.is_ok(), "restored file should be readable");
        assert_eq!(restored.unwrap_or_default(), original_bytes);
    }

    #[test]
    fn import_rejects_a_different_recovery_phrase() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let source_db_path = temp_dir.path().join("source.db");
        let backup_path = temp_dir.path().join("backup.hvb");
        let restored_db_path = temp_dir.path().join("restored.db");
        assert!(write_sample_sqlite(&source_db_path).is_ok());

        let service = BackupServiceImpl::new();
        let (Ok(minted), Ok(other)) = (
            service.generate_recovery_key(),
            service.generate_recovery_key(),
        ) else {
            return;
        };

        assert!(
            service
                .export_hvb_with_recovery_key(
                    &source_db_path,
                    &backup_path,
                    &minted.recovery_phrase
                )
                .is_ok()
        );

        let import_result = service.import_hvb_with_recovery_key(
            &backup_path,
            &other.recovery_phrase,
            &restored_db_path,
        );
        assert!(
            import_result.is_err(),
            "a foreign recovery phrase must not restore the backup"
        );
    }

    #[test]
    fn recovery_verifier_accepts_only_the_matching_phrase() {
        let service = BackupServiceImpl::new();
        let (Ok(minted), Ok(other)) = (
            service.generate_recovery_key(),
            service.generate_recovery_key(),
        ) else {
            return;
        };

        let verifier_result = service.build_recovery_verifier(&minted.recovery_phrase);
        assert!(verifier_result.is_ok(), "verifier build should succeed");
        let verifier = match verifier_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let verifier_bytes = verifier.expose_secret().clone();

        assert_eq!(
            service
                .verify_recovery_phrase(&minted.recovery_phrase, verifier_bytes.as_slice())
                .ok(),
            Some(true)
        );
        assert_eq!(
            service
                .verify_recovery_phrase(&other.recovery_phrase, verifier_bytes.as_slice())
                .ok(),
            Some(false)
        );
    }

    /// The verifier must not leak a value usable as the backup encryption key.
    #[test]
    fn recovery_verifier_is_domain_separated_from_backup_key() {
        let service = BackupServiceImpl::new();
        let Ok(minted) = service.generate_recovery_key() else {
            return;
        };
        let Ok(verifier) = service.build_recovery_verifier(&minted.recovery_phrase) else {
            return;
        };

        let bytes = verifier.expose_secret().clone();
        let salt = &bytes[1..=RECOVERY_SALT_LEN];
        let tag = &bytes[1 + RECOVERY_SALT_LEN..];

        let Ok(backup_key) = BackupServiceImpl::derive_backup_key_from_recovery(
            &minted.recovery_phrase,
            salt,
            HVB_DEFAULT_M_COST_KIB,
            HVB_DEFAULT_T_COST,
            HVB_DEFAULT_P_COST,
        ) else {
            return;
        };

        assert_ne!(backup_key.expose_secret().as_slice(), tag);
    }

    #[test]
    fn verify_recovery_phrase_rejects_malformed_verifiers() {
        let service = BackupServiceImpl::new();
        let Ok(minted) = service.generate_recovery_key() else {
            return;
        };

        assert!(
            service
                .verify_recovery_phrase(&minted.recovery_phrase, &[])
                .is_err(),
            "an empty verifier must be rejected, not silently accepted"
        );

        let mut wrong_version = vec![9_u8];
        wrong_version.extend_from_slice(&[0_u8; RECOVERY_SALT_LEN + RECOVERY_VERIFIER_TAG_LEN]);
        assert!(
            service
                .verify_recovery_phrase(&minted.recovery_phrase, wrong_version.as_slice())
                .is_err(),
            "an unknown verifier version must be rejected"
        );
    }

    /// v1 was a JSON document; it must be refused loudly rather than misparsed.
    #[test]
    fn import_rejects_legacy_v1_json_container() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let legacy_path = temp_dir.path().join("legacy.hvb");
        let restored_path = temp_dir.path().join("restored.db");
        let legacy_json = br#"{"version":1,"kdf":"argon2id","salt_b64":"","nonce_b64":"","ciphertext_b64":"","sha256_hex":"","plaintext_size":0}"#;
        assert!(fs::write(&legacy_path, legacy_json).is_ok());

        let service = BackupServiceImpl::new();
        let Ok(bundle) = service.generate_recovery_key() else {
            return;
        };

        let result = service.import_hvb_with_recovery_key(
            &legacy_path,
            &bundle.recovery_phrase,
            &restored_path,
        );
        assert!(result.is_err(), "a v1 container must be rejected");
        assert!(
            !restored_path.exists(),
            "a rejected import must not create a database"
        );
    }

    #[test]
    fn import_rejects_tampered_header() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let source_path = temp_dir.path().join("source.db");
        let backup_path = temp_dir.path().join("backup.hvb");
        let restored_path = temp_dir.path().join("restored.db");
        assert!(write_sample_sqlite(&source_path).is_ok());

        let service = BackupServiceImpl::new();
        let Ok(bundle) = service.generate_recovery_key() else {
            return;
        };
        assert!(
            service
                .export_hvb_with_recovery_key(&source_path, &backup_path, &bundle.recovery_phrase)
                .is_ok()
        );

        let Ok(mut bytes) = fs::read(&backup_path) else {
            return;
        };
        // Flip a byte inside the JSON header, which AES-GCM covers through AAD.
        let header_byte = HVB_PREFIX_LEN + 4;
        bytes[header_byte] ^= 0x20;
        assert!(fs::write(&backup_path, bytes.as_slice()).is_ok());

        let result = service.import_hvb_with_recovery_key(
            &backup_path,
            &bundle.recovery_phrase,
            &restored_path,
        );
        assert!(
            result.is_err(),
            "an altered header must fail authentication"
        );
    }

    #[test]
    fn import_rejects_weakened_kdf_parameters() {
        let header = HvbHeaderV2 {
            kdf: HVB_KDF_NAME.to_string(),
            m_cost_kib: 8,
            t_cost: HVB_DEFAULT_T_COST,
            p_cost: HVB_DEFAULT_P_COST,
            salt_b64: String::new(),
            nonce_b64: String::new(),
            plaintext_size: 0,
        };
        assert!(
            BackupServiceImpl::validate_kdf_params(&header).is_err(),
            "a downgraded memory cost must be refused"
        );

        let header = HvbHeaderV2 {
            kdf: "pbkdf2".to_string(),
            m_cost_kib: HVB_DEFAULT_M_COST_KIB,
            t_cost: HVB_DEFAULT_T_COST,
            p_cost: HVB_DEFAULT_P_COST,
            salt_b64: String::new(),
            nonce_b64: String::new(),
            plaintext_size: 0,
        };
        assert!(
            BackupServiceImpl::validate_kdf_params(&header).is_err(),
            "an unknown kdf must be refused"
        );
    }

    #[test]
    fn export_leaves_no_partial_file_when_source_is_not_sqlite() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let source_path = temp_dir.path().join("not-a-db.txt");
        let backup_path = temp_dir.path().join("backup.hvb");
        assert!(fs::write(&source_path, b"definitely not sqlite").is_ok());

        let service = BackupServiceImpl::new();
        let Ok(bundle) = service.generate_recovery_key() else {
            return;
        };

        assert!(
            service
                .export_hvb_with_recovery_key(&source_path, &backup_path, &bundle.recovery_phrase)
                .is_err()
        );
        assert!(!backup_path.exists(), "no .hvb should be left behind");
        assert!(
            !temp_dir.path().join(".backup.hvb.partial").exists(),
            "no temporary artifact should be left behind"
        );
    }
}

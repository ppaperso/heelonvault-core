use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::errors::AppError;
use aes_gcm::aead::consts::U12;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use bip39::{Language, Mnemonic};
use gtk4::glib::{Checksum, ChecksumType};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use serde::{Deserialize, Serialize};
use tracing::info;
use zeroize::Zeroizing;

const BACKUP_MAGIC: &[u8; 5] = b"HVBK1";
const SHA256_HEX_LEN: usize = 64;
const BACKUP_NONCE_LEN: usize = 12;
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
const AES_256_KEY_LEN: usize = 32;
const RECOVERY_SALT_LEN: usize = 32;

#[derive(Debug, Clone)]
pub struct RecoveryKeyBundle {
    pub recovery_phrase: SecretString,
}

#[derive(Debug, Serialize, Deserialize)]
struct HvbEncryptedPayload {
    version: u8,
    kdf: String,
    salt_b64: String,
    nonce_b64: String,
    ciphertext_b64: String,
    sha256_hex: String,
    plaintext_size: usize,
}

#[derive(Debug, Clone)]
pub struct BackupMetadata {
    pub sha256_hex: String,
    pub plaintext_size: usize,
}

#[trait_variant::make(BackupService: Send)]
pub trait LocalBackupService {
    fn generate_recovery_key(&self) -> Result<RecoveryKeyBundle, AppError>;
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
    fn export_backup(
        &self,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
        backup_key: SecretBox<Vec<u8>>,
    ) -> Result<BackupMetadata, AppError>;
    fn import_backup(
        &self,
        backup_file_path: &Path,
        target_sqlite_db_path: &Path,
        backup_key: SecretBox<Vec<u8>>,
    ) -> Result<BackupMetadata, AppError>;
}

pub struct BackupServiceImpl;

impl BackupServiceImpl {
    pub fn new() -> Self {
        Self
    }

    fn validate_backup_key(backup_key: &SecretBox<Vec<u8>>) -> Result<(), AppError> {
        if backup_key.expose_secret().len() != AES_256_KEY_LEN {
            return Err(AppError::Validation(format!(
                "backup key must be {AES_256_KEY_LEN} bytes"
            )));
        }

        Ok(())
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
        let mut checksum = Checksum::new(ChecksumType::Sha256)
            .ok_or_else(|| AppError::Validation("failed to initialize SHA-256".to_string()))?;
        checksum.update(bytes);
        checksum
            .string()
            .map(|value| value.to_string())
            .ok_or_else(|| AppError::Validation("failed to finalize SHA-256".to_string()))
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
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        if salt.is_empty() {
            return Err(AppError::Validation(
                "recovery salt must not be empty".to_string(),
            ));
        }

        let params = Params::new(64 * 1024, 3, 1, Some(AES_256_KEY_LEN))
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

    fn parse_hvb_payload(bytes: &[u8]) -> Result<HvbEncryptedPayload, AppError> {
        serde_json::from_slice(bytes)
            .map_err(|err| AppError::Validation(format!("invalid .hvb json payload: {err}")))
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

    fn encrypt_bytes(
        plaintext: &[u8],
        backup_key: &SecretBox<Vec<u8>>,
    ) -> Result<(String, [u8; BACKUP_NONCE_LEN], Vec<u8>), AppError> {
        Self::validate_backup_key(backup_key)?;
        Self::validate_sqlite_bytes(plaintext)?;

        let sha256_hex = Self::sha256_hex(plaintext)?;
        let nonce = Self::generate_nonce()?;
        let nonce_ga: Nonce<U12> = nonce.into();

        let cipher = Aes256Gcm::new_from_slice(backup_key.expose_secret().as_slice())
            .map_err(|err| AppError::Crypto(format!("invalid backup key: {err}")))?;
        let ciphertext = cipher
            .encrypt(&nonce_ga, plaintext)
            .map_err(|_| AppError::Crypto("backup encryption failed".to_string()))?;

        Ok((sha256_hex, nonce, ciphertext))
    }

    fn decrypt_bytes(
        sha256_hex: &str,
        nonce: [u8; BACKUP_NONCE_LEN],
        ciphertext: &[u8],
        backup_key: &SecretBox<Vec<u8>>,
    ) -> Result<Vec<u8>, AppError> {
        Self::validate_backup_key(backup_key)?;

        let cipher = Aes256Gcm::new_from_slice(backup_key.expose_secret().as_slice())
            .map_err(|err| AppError::Crypto(format!("invalid backup key: {err}")))?;
        let nonce_ga: Nonce<U12> = nonce.into();
        let plaintext = cipher
            .decrypt(&nonce_ga, ciphertext)
            .map_err(|_| AppError::Crypto("backup decryption failed".to_string()))?;

        Self::validate_sqlite_bytes(plaintext.as_slice())?;

        let actual_sha256 = Self::sha256_hex(plaintext.as_slice())?;
        if actual_sha256 != sha256_hex {
            return Err(AppError::Validation(
                "backup integrity verification failed before restore".to_string(),
            ));
        }

        Ok(plaintext)
    }

    fn serialize_backup(
        sha256_hex: &str,
        nonce: [u8; BACKUP_NONCE_LEN],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AppError> {
        if sha256_hex.len() != SHA256_HEX_LEN {
            return Err(AppError::Validation(
                "backup SHA-256 digest has invalid length".to_string(),
            ));
        }

        let mut bytes = Vec::with_capacity(
            BACKUP_MAGIC.len() + SHA256_HEX_LEN + BACKUP_NONCE_LEN + ciphertext.len(),
        );
        bytes.extend_from_slice(BACKUP_MAGIC);
        bytes.extend_from_slice(sha256_hex.as_bytes());
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(ciphertext);
        Ok(bytes)
    }

    fn parse_backup(bytes: &[u8]) -> Result<(String, [u8; BACKUP_NONCE_LEN], Vec<u8>), AppError> {
        let minimum_len = BACKUP_MAGIC.len() + SHA256_HEX_LEN + BACKUP_NONCE_LEN + 1;
        if bytes.len() < minimum_len {
            return Err(AppError::Validation("backup file is too short".to_string()));
        }

        if &bytes[0..BACKUP_MAGIC.len()] != BACKUP_MAGIC {
            return Err(AppError::Validation(
                "backup file has an invalid header".to_string(),
            ));
        }

        let digest_start = BACKUP_MAGIC.len();
        let digest_end = digest_start + SHA256_HEX_LEN;
        let nonce_end = digest_end + BACKUP_NONCE_LEN;

        let sha256_hex = std::str::from_utf8(&bytes[digest_start..digest_end])
            .map_err(|_| AppError::Validation("backup digest is not valid UTF-8".to_string()))?
            .to_string();
        if !sha256_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(AppError::Validation(
                "backup digest is not valid hexadecimal".to_string(),
            ));
        }

        let mut nonce = [0_u8; BACKUP_NONCE_LEN];
        nonce.copy_from_slice(&bytes[digest_end..nonce_end]);

        let ciphertext = bytes[nonce_end..].to_vec();
        if ciphertext.is_empty() {
            return Err(AppError::Validation(
                "backup file does not contain ciphertext".to_string(),
            ));
        }

        Ok((sha256_hex, nonce, ciphertext))
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

    fn export_hvb_with_recovery_key(
        &self,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
        recovery_phrase: &SecretString,
    ) -> Result<BackupMetadata, AppError> {
        let sqlite_bytes = fs::read(sqlite_db_path).map_err(AppError::Io)?;
        Self::validate_sqlite_bytes(sqlite_bytes.as_slice())?;

        let salt = Self::generate_recovery_salt()?;
        let backup_key = Self::derive_backup_key_from_recovery(recovery_phrase, &salt)?;
        let (sha256_hex, nonce, ciphertext) =
            Self::encrypt_bytes(sqlite_bytes.as_slice(), &backup_key)?;

        let payload = HvbEncryptedPayload {
            version: 1,
            kdf: "argon2id".to_string(),
            salt_b64: base64::engine::general_purpose::STANDARD.encode(salt),
            nonce_b64: base64::engine::general_purpose::STANDARD.encode(nonce),
            ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
            sha256_hex: sha256_hex.clone(),
            plaintext_size: sqlite_bytes.len(),
        };
        let json_bytes = serde_json::to_vec_pretty(&payload)
            .map_err(|err| AppError::Storage(format!("failed to serialize hvb payload: {err}")))?;

        Self::ensure_parent_exists(backup_file_path)?;
        fs::write(backup_file_path, json_bytes).map_err(AppError::Io)?;
        Self::set_owner_only_file_permissions(backup_file_path)?;

        // Validate the written .hvb artifact end-to-end before reporting success.
        let written_bytes = fs::read(backup_file_path).map_err(AppError::Io)?;
        let written_payload = Self::parse_hvb_payload(written_bytes.as_slice())?;

        if written_payload.version != 1 || written_payload.kdf != "argon2id" {
            return Err(AppError::Validation(
                "written .hvb payload failed format self-check".to_string(),
            ));
        }

        let written_salt = base64::engine::general_purpose::STANDARD
            .decode(written_payload.salt_b64.as_bytes())
            .map_err(|err| AppError::Validation(format!("written .hvb salt is invalid: {err}")))?;
        let written_nonce_vec = base64::engine::general_purpose::STANDARD
            .decode(written_payload.nonce_b64.as_bytes())
            .map_err(|err| AppError::Validation(format!("written .hvb nonce is invalid: {err}")))?;
        let written_ciphertext = base64::engine::general_purpose::STANDARD
            .decode(written_payload.ciphertext_b64.as_bytes())
            .map_err(|err| {
                AppError::Validation(format!("written .hvb ciphertext is invalid: {err}"))
            })?;

        if written_nonce_vec.len() != BACKUP_NONCE_LEN {
            return Err(AppError::Validation(
                "written .hvb nonce has invalid length".to_string(),
            ));
        }

        let mut written_nonce = [0_u8; BACKUP_NONCE_LEN];
        written_nonce.copy_from_slice(written_nonce_vec.as_slice());

        let written_backup_key =
            Self::derive_backup_key_from_recovery(recovery_phrase, written_salt.as_slice())?;
        let restored_bytes = Self::decrypt_bytes(
            written_payload.sha256_hex.as_str(),
            written_nonce,
            written_ciphertext.as_slice(),
            &written_backup_key,
        )?;

        if restored_bytes.len() != sqlite_bytes.len() {
            return Err(AppError::Validation(
                "written .hvb self-check failed: plaintext size mismatch".to_string(),
            ));
        }

        if restored_bytes.as_slice() != sqlite_bytes.as_slice() {
            return Err(AppError::Validation(
                "written .hvb self-check failed: plaintext mismatch".to_string(),
            ));
        }

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
        Mnemonic::parse_in_normalized(Language::English, recovery_phrase.expose_secret())
            .map_err(|err| AppError::Validation(format!("invalid recovery phrase: {err}")))?;

        let backup_bytes = fs::read(backup_file_path).map_err(AppError::Io)?;
        let payload = Self::parse_hvb_payload(backup_bytes.as_slice())?;

        if payload.version != 1 || payload.kdf != "argon2id" {
            return Err(AppError::Validation(
                "unsupported .hvb backup format".to_string(),
            ));
        }

        let salt = base64::engine::general_purpose::STANDARD
            .decode(payload.salt_b64.as_bytes())
            .map_err(|err| AppError::Validation(format!("invalid .hvb salt: {err}")))?;
        let nonce_vec = base64::engine::general_purpose::STANDARD
            .decode(payload.nonce_b64.as_bytes())
            .map_err(|err| AppError::Validation(format!("invalid .hvb nonce: {err}")))?;
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(payload.ciphertext_b64.as_bytes())
            .map_err(|err| AppError::Validation(format!("invalid .hvb ciphertext: {err}")))?;

        if nonce_vec.len() != BACKUP_NONCE_LEN {
            return Err(AppError::Validation(
                "invalid .hvb nonce length".to_string(),
            ));
        }

        let mut nonce = [0_u8; BACKUP_NONCE_LEN];
        nonce.copy_from_slice(nonce_vec.as_slice());

        let backup_key = Self::derive_backup_key_from_recovery(recovery_phrase, salt.as_slice())?;
        let plaintext = Self::decrypt_bytes(
            payload.sha256_hex.as_str(),
            nonce,
            ciphertext.as_slice(),
            &backup_key,
        )?;

        Self::replace_existing_database(new_sqlite_db_path, plaintext.as_slice())?;

        info!(
            file = %backup_file_path.display(),
            destination = %new_sqlite_db_path.display(),
            "Database successfully restored from backup"
        );

        Ok(BackupMetadata {
            sha256_hex: payload.sha256_hex,
            plaintext_size: plaintext.len(),
        })
    }

    fn export_backup(
        &self,
        sqlite_db_path: &Path,
        backup_file_path: &Path,
        backup_key: SecretBox<Vec<u8>>,
    ) -> Result<BackupMetadata, AppError> {
        let sqlite_bytes = fs::read(sqlite_db_path).map_err(AppError::Io)?;

        let (sha256_hex, nonce, ciphertext) =
            Self::encrypt_bytes(sqlite_bytes.as_slice(), &backup_key)?;
        let backup_bytes = Self::serialize_backup(&sha256_hex, nonce, ciphertext.as_slice())?;

        Self::ensure_parent_exists(backup_file_path)?;
        fs::write(backup_file_path, backup_bytes).map_err(AppError::Io)?;
        Self::set_owner_only_file_permissions(backup_file_path)?;

        let exported_sha256 = Self::sha256_hex(sqlite_bytes.as_slice())?;
        if exported_sha256 != sha256_hex {
            return Err(AppError::Validation(
                "backup integrity verification failed after export".to_string(),
            ));
        }

        Ok(BackupMetadata {
            sha256_hex,
            plaintext_size: sqlite_bytes.len(),
        })
    }

    fn import_backup(
        &self,
        backup_file_path: &Path,
        target_sqlite_db_path: &Path,
        backup_key: SecretBox<Vec<u8>>,
    ) -> Result<BackupMetadata, AppError> {
        let backup_bytes = fs::read(backup_file_path).map_err(AppError::Io)?;
        let (sha256_hex, nonce, ciphertext) = Self::parse_backup(backup_bytes.as_slice())?;
        let plaintext =
            Self::decrypt_bytes(&sha256_hex, nonce, ciphertext.as_slice(), &backup_key)?;

        Self::ensure_parent_exists(target_sqlite_db_path)?;
        fs::write(target_sqlite_db_path, plaintext.as_slice()).map_err(AppError::Io)?;
        Self::set_owner_only_file_permissions(target_sqlite_db_path)?;

        let restored_bytes = fs::read(target_sqlite_db_path).map_err(AppError::Io)?;
        let restored_sha256 = Self::sha256_hex(restored_bytes.as_slice())?;
        if restored_sha256 != sha256_hex {
            return Err(AppError::Validation(
                "backup integrity verification failed after restore".to_string(),
            ));
        }

        Ok(BackupMetadata {
            sha256_hex,
            plaintext_size: restored_bytes.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use secrecy::{ExposeSecret, SecretBox};
    use uuid::Uuid;

    use crate::errors::AppError;

    use super::{BackupService, BackupServiceImpl, BACKUP_MAGIC};

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

    #[test]
    fn export_and_import_roundtrip_preserves_sha256_and_bytes() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let source_db_path = temp_dir.path().join("source.db");
        let backup_path = temp_dir.path().join("backup.hvbk");
        let restored_db_path = temp_dir.path().join("restored.db");
        let original_bytes_result = write_sample_sqlite(&source_db_path);
        assert!(original_bytes_result.is_ok(), "sqlite seed should succeed");
        let original_bytes = match original_bytes_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let service = BackupServiceImpl::new();
        let backup_key = SecretBox::new(Box::new(vec![7_u8; 32]));

        let export_result = service.export_backup(
            &source_db_path,
            &backup_path,
            SecretBox::new(Box::new(backup_key.expose_secret().clone())),
        );
        assert!(export_result.is_ok(), "backup export should succeed");
        let export_metadata = match export_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let import_result = service.import_backup(&backup_path, &restored_db_path, backup_key);
        assert!(import_result.is_ok(), "backup import should succeed");
        let import_metadata = match import_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let restored_bytes_result = fs::read(&restored_db_path).map_err(AppError::Io);
        assert!(
            restored_bytes_result.is_ok(),
            "restored file should be readable"
        );
        let restored_bytes = match restored_bytes_result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(export_metadata.sha256_hex, import_metadata.sha256_hex);
        assert_eq!(export_metadata.plaintext_size, original_bytes.len());
        assert_eq!(restored_bytes, original_bytes);
    }

    #[test]
    fn export_rejects_non_sqlite_input() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let source_db_path = temp_dir.path().join("invalid.db");
        let backup_path = temp_dir.path().join("backup.hvbk");
        let seed_result = fs::write(&source_db_path, b"not-a-sqlite-file");
        assert!(seed_result.is_ok(), "invalid seed file should be writable");
        if seed_result.is_err() {
            return;
        }

        let service = BackupServiceImpl::new();
        let export_result = service.export_backup(
            &source_db_path,
            &backup_path,
            SecretBox::new(Box::new(vec![3_u8; 32])),
        );

        assert!(matches!(export_result, Err(AppError::Validation(_))));
    }

    #[test]
    fn import_rejects_tampered_backup_digest() {
        let temp_dir_result = TestTempDir::new();
        assert!(temp_dir_result.is_ok(), "temp dir creation should succeed");
        let temp_dir = match temp_dir_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let source_db_path = temp_dir.path().join("source.db");
        let backup_path = temp_dir.path().join("backup.hvbk");
        let restored_db_path = temp_dir.path().join("restored.db");
        let seed_result = write_sample_sqlite(&source_db_path);
        assert!(seed_result.is_ok(), "sqlite seed should succeed");
        if seed_result.is_err() {
            return;
        }

        let service = BackupServiceImpl::new();
        let backup_key = SecretBox::new(Box::new(vec![9_u8; 32]));
        let export_result = service.export_backup(
            &source_db_path,
            &backup_path,
            SecretBox::new(Box::new(backup_key.expose_secret().clone())),
        );
        assert!(export_result.is_ok(), "backup export should succeed");
        if export_result.is_err() {
            return;
        }

        let backup_bytes_result = fs::read(&backup_path).map_err(AppError::Io);
        assert!(backup_bytes_result.is_ok(), "backup should be readable");
        let mut backup_bytes = match backup_bytes_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let digest_index = BACKUP_MAGIC.len();
        backup_bytes[digest_index] = if backup_bytes[digest_index] == b'a' {
            b'b'
        } else {
            b'a'
        };

        let rewrite_result = fs::write(&backup_path, backup_bytes);
        assert!(rewrite_result.is_ok(), "tampered backup should be writable");
        if rewrite_result.is_err() {
            return;
        }

        let import_result = service.import_backup(&backup_path, &restored_db_path, backup_key);
        assert!(matches!(import_result, Err(AppError::Validation(_))));
    }
}

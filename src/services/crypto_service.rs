use aes_gcm::aead::consts::U12;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use zeroize::Zeroizing;

use crate::errors::AppError;

pub const NONCE_LEN: usize = 12;
pub const AES_256_KEY_LEN: usize = 32;
pub const KDF_SALT_LEN: usize = 32;

#[derive(Debug, Clone)]
pub struct KdfConfig {
    pub memory_cost_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub output_len: usize,
}

impl Default for KdfConfig {
    fn default() -> Self {
        Self {
            memory_cost_kib: 64 * 1024,
            time_cost: 3,
            parallelism: 1,
            output_len: AES_256_KEY_LEN,
        }
    }
}

#[derive(Debug)]
pub struct EncryptedPayload {
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: SecretBox<Vec<u8>>,
}

#[trait_variant::make(CryptoService: Send)]
pub trait LocalCryptoService {
    async fn generate_kdf_salt(&self) -> Result<SecretBox<Vec<u8>>, AppError>;
    async fn derive_key(
        &self,
        password: &SecretString,
        salt: &SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError>;
    async fn encrypt(
        &self,
        plaintext: &SecretBox<Vec<u8>>,
        key: &SecretBox<Vec<u8>>,
    ) -> Result<EncryptedPayload, AppError>;
    async fn decrypt(
        &self,
        payload: &EncryptedPayload,
        key: &SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError>;
}

pub struct CryptoServiceImpl {
    kdf_config: KdfConfig,
}

impl CryptoServiceImpl {
    pub fn new(kdf_config: KdfConfig) -> Self {
        Self { kdf_config }
    }

    pub fn with_defaults() -> Self {
        Self::new(KdfConfig::default())
    }

    fn argon2_instance(&self) -> Result<Argon2<'static>, AppError> {
        let params = Params::new(
            self.kdf_config.memory_cost_kib,
            self.kdf_config.time_cost,
            self.kdf_config.parallelism,
            Some(self.kdf_config.output_len),
        )
        .map_err(|err| AppError::Crypto(format!("invalid argon2 params: {err}")))?;

        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }

    fn generate_nonce() -> Result<[u8; NONCE_LEN], AppError> {
        let mut nonce = [0_u8; NONCE_LEN];
        getrandom::fill(&mut nonce)
            .map_err(|err| AppError::Crypto(format!("nonce generation failed: {err}")))?;
        Ok(nonce)
    }
}

impl Default for CryptoServiceImpl {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl CryptoService for CryptoServiceImpl {
    async fn generate_kdf_salt(&self) -> Result<SecretBox<Vec<u8>>, AppError> {
        let mut salt = Zeroizing::new(vec![0_u8; KDF_SALT_LEN]);
        getrandom::fill(salt.as_mut_slice())
            .map_err(|err| AppError::Crypto(format!("salt generation failed: {err}")))?;
        Ok(SecretBox::new(Box::new(salt.to_vec())))
    }

    async fn derive_key(
        &self,
        password: &SecretString,
        salt: &SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        if salt.expose_secret().is_empty() {
            return Err(AppError::Crypto("salt must not be empty".to_string()));
        }

        let argon2 = self.argon2_instance()?;
        let mut key = Zeroizing::new(vec![0_u8; self.kdf_config.output_len]);
        argon2
            .hash_password_into(
                password.expose_secret().as_bytes(),
                salt.expose_secret().as_slice(),
                key.as_mut_slice(),
            )
            .map_err(|err| AppError::Crypto(format!("argon2id derivation failed: {err}")))?;

        Ok(SecretBox::new(Box::new(key.to_vec())))
    }

    async fn encrypt(
        &self,
        plaintext: &SecretBox<Vec<u8>>,
        key: &SecretBox<Vec<u8>>,
    ) -> Result<EncryptedPayload, AppError> {
        let key_material = Zeroizing::new(key.expose_secret().clone());
        let cipher = Aes256Gcm::new_from_slice(key_material.as_slice())
            .map_err(|err| AppError::Crypto(format!("invalid aes-256 key: {err}")))?;

        let nonce = Self::generate_nonce()?;
        let nonce_ga: Nonce<U12> = nonce.into();
        let ciphertext = cipher
            .encrypt(&nonce_ga, plaintext.expose_secret().as_slice())
            .map_err(|_| AppError::Crypto("encryption failed".to_string()))?;

        Ok(EncryptedPayload {
            nonce,
            ciphertext: SecretBox::new(Box::new(ciphertext)),
        })
    }

    async fn decrypt(
        &self,
        payload: &EncryptedPayload,
        key: &SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        let key_material = Zeroizing::new(key.expose_secret().clone());
        let cipher = Aes256Gcm::new_from_slice(key_material.as_slice())
            .map_err(|err| AppError::Crypto(format!("invalid aes-256 key: {err}")))?;

        let nonce_ga: Nonce<U12> = payload.nonce.into();
        let plaintext = cipher
            .decrypt(&nonce_ga, payload.ciphertext.expose_secret().as_slice())
            .map_err(|_| AppError::Crypto("decryption failed".to_string()))?;

        Ok(SecretBox::new(Box::new(plaintext)))
    }
}

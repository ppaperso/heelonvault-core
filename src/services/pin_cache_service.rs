use std::time::{Duration, Instant};

use aes_gcm::aead::consts::U12;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretBox};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::errors::AppError;
use crate::services::crypto_service::{AES_256_KEY_LEN, KDF_SALT_LEN, NONCE_LEN};

/// How many wrong PIN attempts are allowed before the cache is wiped.
pub const PIN_MAX_FAILED_ATTEMPTS: u8 = 3;

/// Minimum PIN length (digits).
pub const PIN_MIN_LEN: usize = 4;

/// Maximum PIN length (digits).
pub const PIN_MAX_LEN: usize = 8;

/// Argon2id parameters tuned for PIN wrapping.
/// Lower than the master-password KDF (64 MiB / t=3) but still brute-force
/// resistant for a short PIN:
///   ~400 ms/attempt × 10 000 combinations (4-digit) ≈ 1.1 h exhaustive search.
/// Raised from 4 MiB/t=2 to 8 MiB/t=3 to increase offline-brute-force cost
/// after a memory-dump attack (see PR #1 security hardening).
const PIN_KDF_MEMORY_KIB: u32 = 8 * 1024; // 8 MiB
const PIN_KDF_TIME: u32 = 3;
const PIN_KDF_PARALLELISM: u32 = 1;

/// A PIN-protected in-memory cache for the master key.
///
/// - Never persisted to disk.
/// - Disappears on app close, logout, 3 failed attempts, or hard-timeout.
/// - Binds to `user_id` so a cache can never be replayed for a different user.
pub struct PinCache {
    /// AES-256-GCM ciphertext of the master key (wrapped with the PIN-derived key).
    encrypted_master_key: Vec<u8>,
    /// Argon2id salt — 32 random bytes, unique per activation.
    salt: [u8; KDF_SALT_LEN],
    /// AES-GCM nonce — 12 random bytes, unique per activation.
    nonce: [u8; NONCE_LEN],
    /// UTC-equivalent monotonic instant at creation — used for hard-timeout.
    created_at: Instant,
    /// Failed unlock attempts since last successful unlock.
    failed_attempts: u8,
    /// Owner user-id — prevents cache reuse across users.
    user_id: Uuid,
}

impl Drop for PinCache {
    fn drop(&mut self) {
        self.encrypted_master_key.zeroize();
        self.salt.zeroize();
        self.nonce.zeroize();
        self.failed_attempts.zeroize();
    }
}

impl PinCache {
    /// Wrap `master_key` under a key derived from `pin`.
    ///
    /// # Errors
    /// Returns `AppError::Crypto` if key-derivation or encryption fails.
    pub fn wrap(
        master_key: &SecretBox<Vec<u8>>,
        pin: &str,
        user_id: Uuid,
    ) -> Result<Self, AppError> {
        validate_pin(pin)?;

        // Generate random salt.
        let mut salt = [0u8; KDF_SALT_LEN];
        getrandom::fill(&mut salt)
            .map_err(|e| AppError::Crypto(format!("pin salt generation failed: {e}")))?;

        // Derive wrapping key with Argon2id.
        let wrapping_key = derive_pin_key(pin, &salt)?;

        // Generate random nonce.
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce)
            .map_err(|e| AppError::Crypto(format!("pin nonce generation failed: {e}")))?;

        // Encrypt master key with AES-256-GCM.
        let cipher = Aes256Gcm::new_from_slice(wrapping_key.as_slice())
            .map_err(|e| AppError::Crypto(format!("pin aes init failed: {e}")))?;
        let nonce_ga: Nonce<U12> = nonce.into();
        let encrypted_master_key = cipher
            .encrypt(&nonce_ga, master_key.expose_secret().as_slice())
            .map_err(|_| AppError::Crypto("pin master key encryption failed".to_string()))?;

        Ok(Self {
            encrypted_master_key,
            salt,
            nonce,
            created_at: Instant::now(),
            failed_attempts: 0,
            user_id,
        })
    }

    /// Attempt to unwrap the master key using `pin`.
    ///
    /// Returns the master key on success.
    /// Returns `Err` and increments `failed_attempts` on wrong PIN.
    /// The caller **must** check `is_exhausted()` after a failure and wipe the cache.
    pub fn try_unwrap(&mut self, pin: &str) -> Result<Zeroizing<Vec<u8>>, PinUnlockError> {
        if self.is_exhausted() {
            return Err(PinUnlockError::Exhausted);
        }

        match self.decrypt_master_key(pin) {
            Ok(key) => Ok(key),
            Err(_) => {
                self.failed_attempts = self.failed_attempts.saturating_add(1);
                if self.is_exhausted() {
                    Err(PinUnlockError::Exhausted)
                } else {
                    Err(PinUnlockError::WrongPin {
                        remaining: PIN_MAX_FAILED_ATTEMPTS - self.failed_attempts,
                    })
                }
            }
        }
    }

    /// Returns `true` when all attempts are consumed — the cache must be wiped.
    pub fn is_exhausted(&self) -> bool {
        self.failed_attempts >= PIN_MAX_FAILED_ATTEMPTS
    }

    /// Returns `true` if the hard-timeout has elapsed.
    pub fn is_expired(&self, hard_timeout: Duration) -> bool {
        self.created_at.elapsed() >= hard_timeout
    }

    /// The user-id this cache is bound to.
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    // ── private ──────────────────────────────────────────────────────────

    fn decrypt_master_key(&self, pin: &str) -> Result<Zeroizing<Vec<u8>>, AppError> {
        validate_pin(pin)?;

        let wrapping_key = derive_pin_key(pin, &self.salt)?;

        let cipher = Aes256Gcm::new_from_slice(wrapping_key.as_slice())
            .map_err(|e| AppError::Crypto(format!("pin aes init failed: {e}")))?;

        let nonce_ga: Nonce<U12> = self.nonce.into();
        let plaintext = cipher
            .decrypt(&nonce_ga, self.encrypted_master_key.as_slice())
            .map_err(|_| AppError::Crypto("pin decryption failed (wrong PIN)".to_string()))?;

        Ok(Zeroizing::new(plaintext))
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Validate that a PIN contains only ASCII digits and respects length bounds.
pub fn validate_pin(pin: &str) -> Result<(), AppError> {
    if pin.len() < PIN_MIN_LEN || pin.len() > PIN_MAX_LEN {
        return Err(AppError::Crypto(format!(
            "PIN must be {PIN_MIN_LEN}–{PIN_MAX_LEN} digits"
        )));
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Crypto("PIN must contain only digits".to_string()));
    }
    Ok(())
}

fn derive_pin_key(pin: &str, salt: &[u8]) -> Result<Zeroizing<Vec<u8>>, AppError> {
    let params = Params::new(
        PIN_KDF_MEMORY_KIB,
        PIN_KDF_TIME,
        PIN_KDF_PARALLELISM,
        Some(AES_256_KEY_LEN),
    )
    .map_err(|e| AppError::Crypto(format!("invalid pin argon2 params: {e}")))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new(vec![0u8; AES_256_KEY_LEN]);
    argon2
        .hash_password_into(pin.as_bytes(), salt, key.as_mut_slice())
        .map_err(|e| AppError::Crypto(format!("pin argon2id derivation failed: {e}")))?;

    Ok(key)
}

// ── error type ────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum PinUnlockError {
    /// The PIN was wrong; `remaining` is the number of attempts left.
    WrongPin { remaining: u8 },
    /// All attempts consumed — the caller must wipe the cache immediately.
    Exhausted,
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use secrecy::SecretBox;

    fn dummy_master_key() -> SecretBox<Vec<u8>> {
        SecretBox::new(Box::new(vec![0xAB_u8; 32]))
    }

    fn dummy_uuid() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn wrap_then_unwrap_succeeds() {
        let uid = dummy_uuid();
        let mk = dummy_master_key();
        let mut cache = PinCache::wrap(&mk, "123456", uid).expect("wrap failed");
        let recovered = cache.try_unwrap("123456").expect("unwrap failed");
        assert_eq!(recovered.as_slice(), mk.expose_secret().as_slice());
    }

    #[test]
    fn wrong_pin_increments_attempts() {
        let uid = dummy_uuid();
        let mk = dummy_master_key();
        let mut cache = PinCache::wrap(&mk, "1234", uid).expect("wrap failed");
        let err = cache.try_unwrap("9999").unwrap_err();
        assert_eq!(err, PinUnlockError::WrongPin { remaining: 2 });
        assert_eq!(cache.failed_attempts, 1);
    }

    #[test]
    fn three_failures_exhaust_cache() {
        let uid = dummy_uuid();
        let mk = dummy_master_key();
        let mut cache = PinCache::wrap(&mk, "1234", uid).expect("wrap failed");
        for _ in 0..2 {
            let _ = cache.try_unwrap("0000");
        }
        let err = cache.try_unwrap("0000").unwrap_err();
        assert_eq!(err, PinUnlockError::Exhausted);
        assert!(cache.is_exhausted());
    }

    #[test]
    fn wrong_pin_does_not_decode() {
        let uid = dummy_uuid();
        let mk = dummy_master_key();
        let mut cache = PinCache::wrap(&mk, "1234", uid).expect("wrap failed");
        assert!(cache.try_unwrap("4321").is_err());
    }

    #[test]
    fn validate_pin_rejects_short() {
        assert!(validate_pin("123").is_err());
    }

    #[test]
    fn validate_pin_rejects_non_digits() {
        assert!(validate_pin("12ab").is_err());
    }

    #[test]
    fn validate_pin_accepts_boundary_lengths() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("12345678").is_ok());
        assert!(validate_pin("123456789").is_err());
    }

    #[test]
    fn hard_timeout_detects_expiry() {
        let uid = dummy_uuid();
        let mk = dummy_master_key();
        let cache = PinCache::wrap(&mk, "1234", uid).expect("wrap failed");
        assert!(!cache.is_expired(Duration::from_secs(3600)));
        assert!(cache.is_expired(Duration::from_nanos(0)));
    }

    #[test]
    fn user_id_binding_preserved() {
        let uid = dummy_uuid();
        let mk = dummy_master_key();
        let cache = PinCache::wrap(&mk, "1234", uid).expect("wrap failed");
        assert_eq!(cache.user_id(), uid);
    }
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use secrecy::{ExposeSecret, SecretBox, SecretString};

use crate::errors::{AccessDeniedReason, AppError};
use crate::services::crypto_service::CryptoService;

const PASSWORD_ENVELOPE_VERSION: u8 = 1;

struct UserCredentialRecord {
    password_salt: SecretBox<Vec<u8>>,
    password_hash: SecretBox<Vec<u8>>,
}

#[trait_variant::make(AuthService: Send)]
pub trait LocalAuthService {
    async fn create_user(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn verify_password(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<bool, AppError>;
    async fn derive_key_if_valid(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError>;
    async fn change_password(
        &self,
        username: &str,
        current_password: SecretBox<Vec<u8>>,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn upsert_password_envelope(
        &self,
        username: &str,
        password_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError>;
    async fn get_password_envelope(&self, username: &str) -> Result<SecretBox<Vec<u8>>, AppError>;
    fn signal_shutdown(&self);
}

pub struct AuthServiceImpl<TCrypto>
where
    TCrypto: CryptoService + Send + Sync,
{
    crypto_service: TCrypto,
    shutdown_in_progress: AtomicBool,
    credentials: Mutex<HashMap<String, UserCredentialRecord>>,
}

impl<TCrypto> AuthServiceImpl<TCrypto>
where
    TCrypto: CryptoService + Send + Sync,
{
    pub fn new(crypto_service: TCrypto) -> Self {
        Self {
            crypto_service,
            shutdown_in_progress: AtomicBool::new(false),
            credentials: Mutex::new(HashMap::new()),
        }
    }

    fn ensure_not_shutting_down(&self) -> Result<(), AppError> {
        if self.shutdown_in_progress.load(Ordering::SeqCst) {
            return Err(AppError::ShutdownInProgress);
        }
        Ok(())
    }

    fn password_to_secret_string(password: &SecretBox<Vec<u8>>) -> Result<SecretString, AppError> {
        let password_text = std::str::from_utf8(password.expose_secret().as_slice())
            .map_err(|_| AppError::Validation("password must be valid utf-8".to_string()))?;
        Ok(SecretString::new(password_text.to_owned().into_boxed_str()))
    }

    fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
        if left.len() != right.len() {
            return false;
        }

        let mut diff = 0_u8;
        let mut index = 0_usize;
        while index < left.len() {
            diff |= left[index] ^ right[index];
            index += 1;
        }

        diff == 0
    }

    fn encode_password_envelope(record: &UserCredentialRecord) -> SecretBox<Vec<u8>> {
        let salt = record.password_salt.expose_secret();
        let hash = record.password_hash.expose_secret();

        let mut envelope = Vec::with_capacity(1 + 2 + 2 + salt.len() + hash.len());
        envelope.push(PASSWORD_ENVELOPE_VERSION);
        envelope.extend_from_slice(&(salt.len() as u16).to_be_bytes());
        envelope.extend_from_slice(&(hash.len() as u16).to_be_bytes());
        envelope.extend_from_slice(salt);
        envelope.extend_from_slice(hash);

        SecretBox::new(Box::new(envelope))
    }

    fn decode_password_envelope(
        password_envelope: &SecretBox<Vec<u8>>,
    ) -> Result<UserCredentialRecord, AppError> {
        let bytes = password_envelope.expose_secret().as_slice();
        if bytes.len() < 5 {
            return Err(AppError::Validation(
                "invalid password envelope: too short".to_string(),
            ));
        }

        let version = bytes[0];
        if version != PASSWORD_ENVELOPE_VERSION {
            return Err(AppError::Validation(
                "invalid password envelope: unsupported version".to_string(),
            ));
        }

        let salt_len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
        let hash_len = u16::from_be_bytes([bytes[3], bytes[4]]) as usize;
        let expected_len = 5 + salt_len + hash_len;

        if salt_len == 0 || hash_len == 0 || bytes.len() != expected_len {
            return Err(AppError::Validation(
                "invalid password envelope: malformed payload".to_string(),
            ));
        }

        let salt_start = 5;
        let hash_start = salt_start + salt_len;

        let password_salt = SecretBox::new(Box::new(bytes[salt_start..hash_start].to_vec()));
        let password_hash = SecretBox::new(Box::new(bytes[hash_start..].to_vec()));

        Ok(UserCredentialRecord {
            password_salt,
            password_hash,
        })
    }
}

impl<TCrypto> AuthService for AuthServiceImpl<TCrypto>
where
    TCrypto: CryptoService + Send + Sync,
{
    async fn create_user(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        self.ensure_not_shutting_down()?;

        if username.trim().is_empty() {
            return Err(AppError::Validation(
                "username must not be empty".to_string(),
            ));
        }

        {
            let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
            if credentials.contains_key(username) {
                return Err(AppError::Conflict("username already exists".to_string()));
            }
        }

        let secret_password = Self::password_to_secret_string(&password)?;
        let password_salt = self.crypto_service.generate_kdf_salt().await?;
        let password_hash = self
            .crypto_service
            .derive_key(&secret_password, &password_salt)
            .await?;

        let mut credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
        credentials.insert(
            username.to_string(),
            UserCredentialRecord {
                password_salt,
                password_hash,
            },
        );

        Ok(())
    }

    async fn verify_password(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<bool, AppError> {
        self.ensure_not_shutting_down()?;

        let secret_password = Self::password_to_secret_string(&password)?;

        let (password_salt, expected_password_hash) = {
            let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
            let record = credentials.get(username).ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;
            (
                SecretBox::new(Box::new(record.password_salt.expose_secret().clone())),
                record.password_hash.expose_secret().clone(),
            )
        };

        let derived_hash = self
            .crypto_service
            .derive_key(&secret_password, &password_salt)
            .await?;

        Ok(Self::constant_time_eq(
            derived_hash.expose_secret().as_slice(),
            expected_password_hash.as_slice(),
        ))
    }

    async fn change_password(
        &self,
        username: &str,
        current_password: SecretBox<Vec<u8>>,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        self.ensure_not_shutting_down()?;

        if Self::constant_time_eq(
            current_password.expose_secret().as_slice(),
            new_password.expose_secret().as_slice(),
        ) {
            return Err(AppError::Validation(
                "new password must be different from current password".to_string(),
            ));
        }

        let current_secret = Self::password_to_secret_string(&current_password)?;
        let new_secret = Self::password_to_secret_string(&new_password)?;

        let current_salt = {
            let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
            let record = credentials.get(username).ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;

            SecretBox::new(Box::new(record.password_salt.expose_secret().clone()))
        };

        let expected_hash = {
            let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
            let record = credentials.get(username).ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;
            record.password_hash.expose_secret().clone()
        };

        let derived_current = self
            .crypto_service
            .derive_key(&current_secret, &current_salt)
            .await?;
        if !Self::constant_time_eq(
            derived_current.expose_secret().as_slice(),
            expected_hash.as_slice(),
        ) {
            return Err(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ));
        }

        let new_salt = self.crypto_service.generate_kdf_salt().await?;
        let new_hash = self
            .crypto_service
            .derive_key(&new_secret, &new_salt)
            .await?;

        let mut credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
        let record = credentials
            .get_mut(username)
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;
        record.password_salt = new_salt;
        record.password_hash = new_hash;

        Ok(())
    }

    async fn derive_key_if_valid(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        self.ensure_not_shutting_down()?;

        let secret_password = Self::password_to_secret_string(&password)?;

        let (password_salt, expected_password_hash) = {
            let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
            match credentials.get(username) {
                Some(record) => (
                    SecretBox::new(Box::new(record.password_salt.expose_secret().clone())),
                    record.password_hash.expose_secret().clone(),
                ),
                None => return Ok(None),
            }
        };

        let derived_hash = self
            .crypto_service
            .derive_key(&secret_password, &password_salt)
            .await?;

        if Self::constant_time_eq(
            derived_hash.expose_secret().as_slice(),
            expected_password_hash.as_slice(),
        ) {
            Ok(Some(derived_hash))
        } else {
            Ok(None)
        }
    }

    async fn upsert_password_envelope(
        &self,
        username: &str,
        password_envelope: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        self.ensure_not_shutting_down()?;

        if username.trim().is_empty() {
            return Err(AppError::Validation(
                "username must not be empty".to_string(),
            ));
        }

        let record = Self::decode_password_envelope(&password_envelope)?;

        let mut credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
        credentials.insert(username.to_string(), record);
        Ok(())
    }

    async fn get_password_envelope(&self, username: &str) -> Result<SecretBox<Vec<u8>>, AppError> {
        self.ensure_not_shutting_down()?;

        let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
        let record = credentials.get(username).ok_or(AppError::Authorization(
            AccessDeniedReason::InvalidCredentials,
        ))?;

        Ok(Self::encode_password_envelope(record))
    }

    fn signal_shutdown(&self) {
        self.shutdown_in_progress.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{AuthService, AuthServiceImpl};
    use crate::errors::AppError;
    use crate::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretBox;

    #[tokio::test]
    async fn password_envelope_roundtrip_keeps_credentials_valid() {
        let source = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
        let create_result = source
            .create_user("alice", SecretBox::new(Box::new(b"ChangeMe#2026".to_vec())))
            .await;
        assert!(create_result.is_ok(), "source create_user should succeed");
        if create_result.is_err() {
            return;
        }

        let envelope_result = source.get_password_envelope("alice").await;
        assert!(
            envelope_result.is_ok(),
            "export password envelope should succeed"
        );
        let envelope = match envelope_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let restored = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
        let import_result = restored.upsert_password_envelope("alice", envelope).await;
        assert!(
            import_result.is_ok(),
            "import password envelope should succeed"
        );
        if import_result.is_err() {
            return;
        }

        let verify_ok = restored
            .verify_password("alice", SecretBox::new(Box::new(b"ChangeMe#2026".to_vec())))
            .await;
        assert!(
            matches!(verify_ok, Ok(true)),
            "restored credentials should validate"
        );
    }

    #[tokio::test]
    async fn change_password_persisted_envelope_works_after_reload() {
        let source = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
        let create_result = source
            .create_user(
                "bob",
                SecretBox::new(Box::new(b"OldPassword#2026".to_vec())),
            )
            .await;
        assert!(create_result.is_ok(), "create_user should succeed");
        if create_result.is_err() {
            return;
        }

        let change_result = source
            .change_password(
                "bob",
                SecretBox::new(Box::new(b"OldPassword#2026".to_vec())),
                SecretBox::new(Box::new(b"NewPassword#2026".to_vec())),
            )
            .await;
        assert!(change_result.is_ok(), "change_password should succeed");
        if change_result.is_err() {
            return;
        }

        let envelope_result = source.get_password_envelope("bob").await;
        assert!(
            envelope_result.is_ok(),
            "export password envelope should succeed"
        );
        let envelope = match envelope_result {
            Ok(value) => value,
            Err(_) => return,
        };

        let restored = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
        let import_result = restored.upsert_password_envelope("bob", envelope).await;
        assert!(
            import_result.is_ok(),
            "import password envelope should succeed"
        );
        if import_result.is_err() {
            return;
        }

        let old_password_check = restored
            .verify_password(
                "bob",
                SecretBox::new(Box::new(b"OldPassword#2026".to_vec())),
            )
            .await;
        assert!(
            matches!(old_password_check, Ok(false)),
            "old password should be rejected"
        );

        let new_password_check = restored
            .verify_password(
                "bob",
                SecretBox::new(Box::new(b"NewPassword#2026".to_vec())),
            )
            .await;
        assert!(
            matches!(new_password_check, Ok(true)),
            "new password should be accepted"
        );
    }

    #[tokio::test]
    async fn change_password_rejects_identical_value() {
        let service = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
        let create_result = service
            .create_user(
                "eve",
                SecretBox::new(Box::new(b"SamePassword#2026".to_vec())),
            )
            .await;
        assert!(create_result.is_ok(), "create_user should succeed");
        if create_result.is_err() {
            return;
        }

        let result = service
            .change_password(
                "eve",
                SecretBox::new(Box::new(b"SamePassword#2026".to_vec())),
                SecretBox::new(Box::new(b"SamePassword#2026".to_vec())),
            )
            .await;

        assert!(
            matches!(result, Err(AppError::Validation(_))),
            "identical password replacement must be rejected"
        );
    }
}

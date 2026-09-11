use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use secrecy::{ExposeSecret, SecretBox, SecretString};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::errors::{AccessDeniedReason, AppError};
use crate::services::crypto_service::{CryptoService, EncryptedPayload, NONCE_LEN};

const LEGACY_ENVELOPE_VERSION: u8 = 1;
const ACCOUNT_KEY_ENVELOPE_VERSION: u8 = 2;
const ENVELOPE_HEADER_LEN: usize = 5;

/// Content of `users.password_envelope`: `version || salt_len || body_len || salt || body`.
pub enum PasswordEnvelope {
    /// v1: the body is the Argon2id output itself, which also wraps the vault keys, so the
    /// database alone opens them. Kept to migrate, and for multi-account vaults.
    Legacy {
        salt: SecretBox<Vec<u8>>,
        master_key: SecretBox<Vec<u8>>,
    },
    /// v2: the body is the account key, encrypted under the Argon2id output.
    AccountKey {
        salt: SecretBox<Vec<u8>>,
        wrapped_account_key: EncryptedPayload,
    },
}

impl PasswordEnvelope {
    pub fn is_account_key(&self) -> bool {
        matches!(self, Self::AccountKey { .. })
    }
}

pub fn encode_password_envelope(envelope: &PasswordEnvelope) -> SecretBox<Vec<u8>> {
    let (version, salt, body) = match envelope {
        PasswordEnvelope::Legacy { salt, master_key } => (
            LEGACY_ENVELOPE_VERSION,
            salt,
            Zeroizing::new(master_key.expose_secret().clone()),
        ),
        PasswordEnvelope::AccountKey {
            salt,
            wrapped_account_key,
        } => {
            let ciphertext = wrapped_account_key.ciphertext.expose_secret();
            let mut body = Zeroizing::new(Vec::with_capacity(NONCE_LEN + ciphertext.len()));
            body.extend_from_slice(&wrapped_account_key.nonce);
            body.extend_from_slice(ciphertext);
            (ACCOUNT_KEY_ENVELOPE_VERSION, salt, body)
        }
    };

    let salt = salt.expose_secret();
    let mut bytes = Vec::with_capacity(ENVELOPE_HEADER_LEN + salt.len() + body.len());
    bytes.push(version);
    bytes.extend_from_slice(&(salt.len() as u16).to_be_bytes());
    bytes.extend_from_slice(&(body.len() as u16).to_be_bytes());
    bytes.extend_from_slice(salt);
    bytes.extend_from_slice(body.as_slice());
    SecretBox::new(Box::new(bytes))
}

pub fn decode_password_envelope(
    envelope: &SecretBox<Vec<u8>>,
) -> Result<PasswordEnvelope, AppError> {
    let bytes = envelope.expose_secret().as_slice();
    if bytes.len() < ENVELOPE_HEADER_LEN {
        return Err(AppError::Validation(
            "invalid password envelope: too short".to_string(),
        ));
    }

    let salt_len = u16::from_be_bytes([bytes[1], bytes[2]]) as usize;
    let body_len = u16::from_be_bytes([bytes[3], bytes[4]]) as usize;
    if salt_len == 0 || body_len == 0 || bytes.len() != ENVELOPE_HEADER_LEN + salt_len + body_len {
        return Err(AppError::Validation(
            "invalid password envelope: malformed payload".to_string(),
        ));
    }

    let body_start = ENVELOPE_HEADER_LEN + salt_len;
    let salt = SecretBox::new(Box::new(bytes[ENVELOPE_HEADER_LEN..body_start].to_vec()));
    let body = &bytes[body_start..];

    match bytes[0] {
        LEGACY_ENVELOPE_VERSION => Ok(PasswordEnvelope::Legacy {
            salt,
            master_key: SecretBox::new(Box::new(body.to_vec())),
        }),
        ACCOUNT_KEY_ENVELOPE_VERSION if body.len() > NONCE_LEN => {
            let mut nonce = [0_u8; NONCE_LEN];
            nonce.copy_from_slice(&body[..NONCE_LEN]);
            Ok(PasswordEnvelope::AccountKey {
                salt,
                wrapped_account_key: EncryptedPayload {
                    nonce,
                    ciphertext: SecretBox::new(Box::new(body[NONCE_LEN..].to_vec())),
                },
            })
        }
        ACCOUNT_KEY_ENVELOPE_VERSION => Err(AppError::Validation(
            "invalid password envelope: malformed payload".to_string(),
        )),
        _ => Err(AppError::Validation(
            "invalid password envelope: unsupported version".to_string(),
        )),
    }
}

/// Encrypts `account_key` under a key derived from `password` with a fresh salt.
pub async fn wrap_account_key<C>(
    crypto: &C,
    password: &SecretString,
    account_key: &SecretBox<Vec<u8>>,
) -> Result<PasswordEnvelope, AppError>
where
    C: CryptoService + Send + Sync,
{
    let salt = crypto.generate_kdf_salt().await?;
    let kek = crypto.derive_key(password, &salt).await?;
    let wrapped_account_key = crypto.encrypt(account_key, &kek).await?;
    Ok(PasswordEnvelope::AccountKey {
        salt,
        wrapped_account_key,
    })
}

/// The key `password` opens: the account key (v2) or the legacy master key (v1).
pub async fn unlock_password_envelope<C>(
    crypto: &C,
    envelope: &PasswordEnvelope,
    password: &SecretString,
) -> Result<Option<SecretBox<Vec<u8>>>, AppError>
where
    C: CryptoService + Send + Sync,
{
    match envelope {
        PasswordEnvelope::Legacy { salt, master_key } => {
            let derived = crypto.derive_key(password, salt).await?;
            let matches = derived
                .expose_secret()
                .as_slice()
                .ct_eq(master_key.expose_secret().as_slice());
            Ok(bool::from(matches).then_some(derived))
        }
        PasswordEnvelope::AccountKey {
            salt,
            wrapped_account_key,
        } => {
            let kek = crypto.derive_key(password, salt).await?;
            Ok(crypto.decrypt(wrapped_account_key, &kek).await.ok())
        }
    }
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
    /// Encoded password envelopes, by username.
    credentials: Mutex<HashMap<String, SecretBox<Vec<u8>>>>,
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

    fn stored_envelope(&self, username: &str) -> Result<Option<PasswordEnvelope>, AppError> {
        let stored = {
            let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
            credentials
                .get(username)
                .map(|envelope| SecretBox::new(Box::new(envelope.expose_secret().clone())))
        };
        stored.as_ref().map(decode_password_envelope).transpose()
    }

    fn store_envelope(&self, username: &str, envelope: &PasswordEnvelope) -> Result<(), AppError> {
        let mut credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
        credentials.insert(username.to_string(), encode_password_envelope(envelope));
        Ok(())
    }
}

impl<TCrypto> AuthService for AuthServiceImpl<TCrypto>
where
    TCrypto: CryptoService + Send + Sync,
{
    /// New accounts use the legacy envelope so multi-account vaults can still share vaults;
    /// single-account vaults move to an account key at bootstrap or at their next login.
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
            // Nobody could share a vault with the existing account-key account.
            if credentials.values().any(|envelope| {
                envelope.expose_secret().first() == Some(&ACCOUNT_KEY_ENVELOPE_VERSION)
            }) {
                return Err(AppError::SingleAccountVault);
            }
        }

        let secret_password = Self::password_to_secret_string(&password)?;
        let salt = self.crypto_service.generate_kdf_salt().await?;
        let master_key = self
            .crypto_service
            .derive_key(&secret_password, &salt)
            .await?;

        self.store_envelope(username, &PasswordEnvelope::Legacy { salt, master_key })
    }

    async fn verify_password(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<bool, AppError> {
        self.ensure_not_shutting_down()?;

        let envelope = self
            .stored_envelope(username)?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;
        let password = Self::password_to_secret_string(&password)?;
        Ok(
            unlock_password_envelope(&self.crypto_service, &envelope, &password)
                .await?
                .is_some(),
        )
    }

    async fn change_password(
        &self,
        username: &str,
        current_password: SecretBox<Vec<u8>>,
        new_password: SecretBox<Vec<u8>>,
    ) -> Result<(), AppError> {
        self.ensure_not_shutting_down()?;

        if bool::from(
            current_password
                .expose_secret()
                .as_slice()
                .ct_eq(new_password.expose_secret().as_slice()),
        ) {
            return Err(AppError::Validation(
                "new password must be different from current password".to_string(),
            ));
        }

        let current_secret = Self::password_to_secret_string(&current_password)?;
        let new_secret = Self::password_to_secret_string(&new_password)?;

        let envelope = self
            .stored_envelope(username)?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;
        let key = unlock_password_envelope(&self.crypto_service, &envelope, &current_secret)
            .await?
            .ok_or(AppError::Authorization(
                AccessDeniedReason::InvalidCredentials,
            ))?;

        let replacement = match envelope {
            PasswordEnvelope::AccountKey { .. } => {
                wrap_account_key(&self.crypto_service, &new_secret, &key).await?
            }
            PasswordEnvelope::Legacy { .. } => {
                let salt = self.crypto_service.generate_kdf_salt().await?;
                let master_key = self.crypto_service.derive_key(&new_secret, &salt).await?;
                PasswordEnvelope::Legacy { salt, master_key }
            }
        };
        self.store_envelope(username, &replacement)
    }

    async fn derive_key_if_valid(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        self.ensure_not_shutting_down()?;

        let Some(envelope) = self.stored_envelope(username)? else {
            return Ok(None);
        };
        let password = Self::password_to_secret_string(&password)?;
        unlock_password_envelope(&self.crypto_service, &envelope, &password).await
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

        decode_password_envelope(&password_envelope)?;

        let mut credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
        credentials.insert(username.to_string(), password_envelope);
        Ok(())
    }

    async fn get_password_envelope(&self, username: &str) -> Result<SecretBox<Vec<u8>>, AppError> {
        self.ensure_not_shutting_down()?;

        let credentials = self.credentials.lock().map_err(|_| AppError::Internal)?;
        let envelope = credentials.get(username).ok_or(AppError::Authorization(
            AccessDeniedReason::InvalidCredentials,
        ))?;
        Ok(SecretBox::new(Box::new(envelope.expose_secret().clone())))
    }

    fn signal_shutdown(&self) {
        self.shutdown_in_progress.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::disallowed_methods)]
    use super::{
        AuthService, AuthServiceImpl, PasswordEnvelope, decode_password_envelope,
        encode_password_envelope, wrap_account_key,
    };
    use crate::errors::AppError;
    use crate::services::account_key::generate_account_key;
    use crate::services::crypto_service::{CryptoServiceImpl, KdfConfig};
    use secrecy::{ExposeSecret, SecretBox, SecretString};

    fn fast_crypto() -> CryptoServiceImpl {
        CryptoServiceImpl::new(KdfConfig {
            memory_cost_kib: 1024,
            time_cost: 1,
            parallelism: 1,
            output_len: 32,
        })
    }

    fn secret(value: &str) -> SecretBox<Vec<u8>> {
        SecretBox::new(Box::new(value.as_bytes().to_vec()))
    }

    /// Credentials loaded the way startup loads them, for an account-key account.
    async fn account_key_service(password: &str) -> (AuthServiceImpl<CryptoServiceImpl>, Vec<u8>) {
        let account_key = generate_account_key().expect("account key");
        let envelope = wrap_account_key(
            &fast_crypto(),
            &SecretString::new(password.into()),
            &account_key,
        )
        .await
        .expect("wrap account key");
        let service = AuthServiceImpl::new(fast_crypto());
        service
            .upsert_password_envelope("alice", encode_password_envelope(&envelope))
            .await
            .expect("load credentials");
        (service, account_key.expose_secret().clone())
    }

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

    #[tokio::test]
    async fn account_key_opens_only_with_its_password() {
        let (service, account_key) = account_key_service("correct horse battery").await;

        let opened = service
            .derive_key_if_valid("alice", secret("correct horse battery"))
            .await
            .expect("derive")
            .expect("the password opens the account key");
        assert_eq!(opened.expose_secret(), &account_key);

        assert!(
            service
                .derive_key_if_valid("alice", secret("wrong horse battery"))
                .await
                .expect("derive")
                .is_none()
        );
        assert!(
            !service
                .verify_password("alice", secret("wrong horse battery"))
                .await
                .expect("verify")
        );
    }

    #[tokio::test]
    async fn changing_the_password_keeps_the_account_key() {
        let (service, account_key) = account_key_service("first password here").await;

        service
            .change_password(
                "alice",
                secret("first password here"),
                secret("second password here"),
            )
            .await
            .expect("change password");

        let opened = service
            .derive_key_if_valid("alice", secret("second password here"))
            .await
            .expect("derive")
            .expect("new password opens the account key");
        assert_eq!(opened.expose_secret(), &account_key);
        assert!(
            service
                .derive_key_if_valid("alice", secret("first password here"))
                .await
                .expect("derive")
                .is_none()
        );
    }

    #[tokio::test]
    async fn account_key_envelope_survives_encoding() {
        let (service, _) = account_key_service("some password value").await;
        let encoded = service
            .get_password_envelope("alice")
            .await
            .expect("envelope");
        let decoded = decode_password_envelope(&encoded).expect("decode");
        assert!(decoded.is_account_key());
        assert_eq!(
            encode_password_envelope(&decoded).expose_secret(),
            encoded.expose_secret()
        );
        assert!(matches!(decoded, PasswordEnvelope::AccountKey { .. }));
    }

    #[tokio::test]
    async fn no_second_account_next_to_an_account_key_account() {
        let (service, _) = account_key_service("some password value").await;

        let result = service
            .create_user("bob", secret("another password value"))
            .await;

        assert!(matches!(result, Err(AppError::SingleAccountVault)));
    }
}

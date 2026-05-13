use std::collections::HashMap;
use std::sync::Mutex;

use image::ImageFormat;
use qrcode::QrCode;
use secrecy::{ExposeSecret, SecretBox};
use sqlx::{Row, SqlitePool};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

use crate::errors::{AccessDeniedReason, AppError};
use crate::services::auth_service::AuthService;
use crate::services::crypto_service::{CryptoService, EncryptedPayload, NONCE_LEN};

const TOTP_DIGITS: usize = 6;
const TOTP_STEP: u64 = 30;
const TOTP_SKEW: u8 = 1;
const TOTP_REPLAY_TTL_SECS: i64 = (TOTP_STEP as i64) * 3;
const PASSWORD_ENVELOPE_VERSION: u8 = 1;
const LEGACY_DEV_MASTER_KEY_BYTE: u8 = 0x41;
const LEGACY_DEV_MASTER_KEY_LEN: usize = 32;

#[derive(Debug, Clone)]
pub struct TotpSetupPayload {
    pub base32_secret: String,
    pub otpauth_url: String,
    pub qr_png: Vec<u8>,
}

#[trait_variant::make(TotpService: Send)]
pub trait LocalTotpService {
    async fn is_totp_enabled_for_user_id(&self, user_id: Uuid) -> Result<bool, AppError>;
    async fn is_totp_enabled_for_username(&self, username: &str) -> Result<bool, AppError>;
    fn create_setup_payload(&self, account_name: &str) -> Result<TotpSetupPayload, AppError>;
    fn verify_setup_code(
        &self,
        account_name: &str,
        base32_secret: &str,
        code: &str,
    ) -> Result<bool, AppError>;
    async fn enable_totp(
        &self,
        user_id: Uuid,
        username: &str,
        base32_secret: &str,
        code: &str,
    ) -> Result<(), AppError>;
    async fn disable_totp(&self, user_id: Uuid) -> Result<(), AppError>;
    async fn verify_login_totp(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
        code: &str,
    ) -> Result<bool, AppError>;
}

pub struct SqliteTotpService<TAuth, TCrypto>
where
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    pool: SqlitePool,
    auth_service: std::sync::Arc<TAuth>,
    crypto_service: TCrypto,
    issuer: String,
    replay_guard: Mutex<HashMap<String, i64>>,
}

impl<TAuth, TCrypto> SqliteTotpService<TAuth, TCrypto>
where
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    pub fn new(
        pool: SqlitePool,
        auth_service: std::sync::Arc<TAuth>,
        crypto_service: TCrypto,
        issuer: impl Into<String>,
    ) -> Self {
        Self {
            pool,
            auth_service,
            crypto_service,
            issuer: issuer.into(),
            replay_guard: Mutex::new(HashMap::new()),
        }
    }

    fn replay_key(username: &str, code: &str) -> String {
        format!("{username}:{code}")
    }

    fn reject_replay_and_record(
        &self,
        username: &str,
        code: &str,
        now_ts: i64,
    ) -> Result<bool, AppError> {
        let mut guard = self.replay_guard.lock().map_err(|_| AppError::Internal)?;

        guard.retain(|_, expires_at| *expires_at > now_ts);

        let key = Self::replay_key(username, code);
        if guard.contains_key(&key) {
            return Ok(false);
        }

        guard.insert(key, now_ts + TOTP_REPLAY_TTL_SECS);
        Ok(true)
    }

    fn build_totp(&self, account_name: &str, base32_secret: &str) -> Result<TOTP, AppError> {
        let secret_bytes = Secret::Encoded(base32_secret.to_string())
            .to_bytes()
            .map_err(|error| AppError::Validation(format!("invalid TOTP secret: {error}")))?;

        TOTP::new(
            Algorithm::SHA1,
            TOTP_DIGITS,
            TOTP_SKEW,
            TOTP_STEP,
            secret_bytes,
            Some(self.issuer.clone()),
            account_name.to_string(),
        )
        .map_err(|error| AppError::Validation(format!("invalid TOTP setup: {error}")))
    }

    fn is_valid_totp_code(code: &str) -> bool {
        code.len() == TOTP_DIGITS && code.chars().all(|character| character.is_ascii_digit())
    }

    fn serialize_envelope(payload: &EncryptedPayload) -> SecretBox<Vec<u8>> {
        let mut bytes = Vec::with_capacity(NONCE_LEN + payload.ciphertext.expose_secret().len());
        bytes.extend_from_slice(&payload.nonce);
        bytes.extend_from_slice(payload.ciphertext.expose_secret().as_slice());
        SecretBox::new(Box::new(bytes))
    }

    fn deserialize_envelope(bytes: &SecretBox<Vec<u8>>) -> Result<EncryptedPayload, AppError> {
        if bytes.expose_secret().len() <= NONCE_LEN {
            return Err(AppError::Storage("invalid totp_secret payload".to_string()));
        }

        let mut nonce = [0_u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes.expose_secret()[0..NONCE_LEN]);

        Ok(EncryptedPayload {
            nonce,
            ciphertext: SecretBox::new(Box::new(bytes.expose_secret()[NONCE_LEN..].to_vec())),
        })
    }

    fn derive_key_from_password_envelope(
        password_envelope: &SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
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

    async fn load_totp_secret_by_username(
        &self,
        username: &str,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        let row_opt = sqlx::query("SELECT totp_secret FROM users WHERE username = ?1")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        match row_opt {
            Some(row) => {
                let bytes: Option<Vec<u8>> = row.try_get("totp_secret")?;
                Ok(bytes.map(|value| SecretBox::new(Box::new(value))))
            }
            None => Ok(None),
        }
    }

    async fn load_totp_secret_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        let row_opt = sqlx::query("SELECT totp_secret FROM users WHERE id = ?1")
            .bind(user_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row_opt {
            Some(row) => {
                let bytes: Option<Vec<u8>> = row.try_get("totp_secret")?;
                Ok(bytes.map(|value| SecretBox::new(Box::new(value))))
            }
            None => Ok(None),
        }
    }
}

impl<TAuth, TCrypto> TotpService for SqliteTotpService<TAuth, TCrypto>
where
    TAuth: AuthService + Send + Sync,
    TCrypto: CryptoService + Send + Sync,
{
    async fn is_totp_enabled_for_user_id(&self, user_id: Uuid) -> Result<bool, AppError> {
        Ok(self.load_totp_secret_by_user_id(user_id).await?.is_some())
    }

    async fn is_totp_enabled_for_username(&self, username: &str) -> Result<bool, AppError> {
        Ok(self.load_totp_secret_by_username(username).await?.is_some())
    }

    fn create_setup_payload(&self, account_name: &str) -> Result<TotpSetupPayload, AppError> {
        if account_name.trim().is_empty() {
            return Err(AppError::Validation(
                "account name is required for TOTP setup".to_string(),
            ));
        }

        let secret = Secret::generate_secret();
        let base32_secret = match secret.to_encoded() {
            Secret::Encoded(value) => value,
            Secret::Raw(_) => {
                return Err(AppError::Validation(
                    "failed to encode TOTP secret".to_string(),
                ));
            }
        };
        let totp = self.build_totp(account_name, base32_secret.as_str())?;
        let otpauth_url = totp.get_url();

        let qr_code = QrCode::new(otpauth_url.as_bytes())
            .map_err(|error| AppError::Validation(format!("failed to generate QR: {error}")))?;
        let img = qr_code
            .render::<image::Luma<u8>>()
            .min_dimensions(200, 200)
            .build();
        let mut qr_png: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut qr_png), ImageFormat::Png)
            .map_err(|error| {
                AppError::Validation(format!("failed to encode QR as PNG: {error}"))
            })?;

        Ok(TotpSetupPayload {
            base32_secret,
            otpauth_url,
            qr_png,
        })
    }

    fn verify_setup_code(
        &self,
        account_name: &str,
        base32_secret: &str,
        code: &str,
    ) -> Result<bool, AppError> {
        if !Self::is_valid_totp_code(code) {
            return Ok(false);
        }

        let totp = self.build_totp(account_name, base32_secret)?;
        totp.check_current(code)
            .map_err(|error| AppError::Validation(format!("failed to verify TOTP code: {error}")))
    }

    async fn enable_totp(
        &self,
        user_id: Uuid,
        username: &str,
        base32_secret: &str,
        code: &str,
    ) -> Result<(), AppError> {
        if username.trim().is_empty() {
            return Err(AppError::Validation(
                "username must not be empty".to_string(),
            ));
        }

        let password_envelope = self.auth_service.get_password_envelope(username).await?;
        let key = Self::derive_key_from_password_envelope(&password_envelope)?;

        let is_code_valid = TotpService::verify_setup_code(self, username, base32_secret, code)?;
        if !is_code_valid {
            return Err(AppError::Authorization(AccessDeniedReason::InvalidTotpCode));
        }

        let encrypted = self
            .crypto_service
            .encrypt(
                &SecretBox::new(Box::new(base32_secret.as_bytes().to_vec())),
                &key,
            )
            .await?;
        let envelope = Self::serialize_envelope(&encrypted);

        let result = sqlx::query("UPDATE users SET totp_secret = ?1 WHERE id = ?2")
            .bind(envelope.expose_secret().as_slice())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "user not found for TOTP activation".to_string(),
            ));
        }

        Ok(())
    }

    async fn disable_totp(&self, user_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE users SET totp_secret = NULL WHERE id = ?1")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::Storage(
                "user not found for TOTP deactivation".to_string(),
            ));
        }

        Ok(())
    }

    async fn verify_login_totp(
        &self,
        username: &str,
        password: SecretBox<Vec<u8>>,
        code: &str,
    ) -> Result<bool, AppError> {
        if !Self::is_valid_totp_code(code) {
            return Ok(false);
        }

        let encrypted_secret_opt = self.load_totp_secret_by_username(username).await?;
        let Some(encrypted_secret) = encrypted_secret_opt else {
            return Ok(true);
        };

        let key_opt = self
            .auth_service
            .derive_key_if_valid(username, password)
            .await?;
        let Some(key) = key_opt else {
            return Ok(false);
        };

        let payload = Self::deserialize_envelope(&encrypted_secret)?;
        let decrypted = match self.crypto_service.decrypt(&payload, &key).await {
            Ok(value) => value,
            Err(_) => {
                let legacy_key = SecretBox::new(Box::new(vec![
                    LEGACY_DEV_MASTER_KEY_BYTE;
                    LEGACY_DEV_MASTER_KEY_LEN
                ]));
                let legacy_decrypted =
                    match self.crypto_service.decrypt(&payload, &legacy_key).await {
                        Ok(value) => value,
                        Err(_) => return Ok(false),
                    };

                let legacy_secret = String::from_utf8(legacy_decrypted.expose_secret().clone())
                    .map_err(|_| {
                        AppError::Validation("invalid decrypted TOTP secret".to_string())
                    })?;
                let legacy_totp = self.build_totp(username, legacy_secret.as_str())?;
                let is_valid = legacy_totp.check_current(code).map_err(|error| {
                    AppError::Validation(format!("failed to verify login TOTP code: {error}"))
                })?;

                if is_valid {
                    let reencrypted = self
                        .crypto_service
                        .encrypt(&SecretBox::new(Box::new(legacy_secret.into_bytes())), &key)
                        .await?;
                    let envelope = Self::serialize_envelope(&reencrypted);
                    let _ = sqlx::query("UPDATE users SET totp_secret = ?1 WHERE username = ?2")
                        .bind(envelope.expose_secret().as_slice())
                        .bind(username)
                        .execute(&self.pool)
                        .await;
                }

                return Ok(is_valid);
            }
        };
        let base32_secret = String::from_utf8(decrypted.expose_secret().clone())
            .map_err(|_| AppError::Validation("invalid decrypted TOTP secret".to_string()))?;

        let totp = self.build_totp(username, base32_secret.as_str())?;
        let is_valid = totp.check_current(code).map_err(|error| {
            AppError::Validation(format!("failed to verify login TOTP code: {error}"))
        })?;

        if !is_valid {
            return Ok(false);
        }

        let now_ts = chrono::Utc::now().timestamp();
        self.reject_replay_and_record(username, code, now_ts)
    }
}

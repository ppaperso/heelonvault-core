#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use heelonvault_rust::errors::AppError;
use heelonvault_rust::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_rust::services::crypto_service::CryptoServiceImpl;
use heelonvault_rust::services::totp_service::{SqliteTotpService, TotpService};
use secrecy::SecretBox;
use sqlx::{Row, SqlitePool};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

fn mutate_totp_code(code: &str) -> String {
    let mut chars: Vec<char> = code.chars().collect();
    if let Some(first) = chars.first_mut() {
        *first = if *first == '0' { '1' } else { '0' };
    }
    chars.into_iter().collect()
}

fn build_current_totp_code(username: &str, base32_secret: &str) -> Result<String, AppError> {
    let secret_bytes = Secret::Encoded(base32_secret.to_string())
        .to_bytes()
        .map_err(|error| AppError::Validation(format!("invalid TOTP secret in test: {error}")))?;

    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some("HeelonVault".to_string()),
        username.to_string(),
    )
    .map_err(|error| AppError::Validation(format!("invalid TOTP config in test: {error}")))?;

    totp.generate_current().map_err(|error| {
        AppError::Validation(format!("failed to generate TOTP code in test: {error}"))
    })
}

#[tokio::test]
async fn enable_totp_requires_valid_code_without_password_prompt() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("sqlite memory pool should initialize");

    sqlx::query(
        "CREATE TABLE users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            totp_secret BLOB
        )",
    )
    .execute(&pool)
    .await
    .expect("users table should be created");

    let user_id = Uuid::new_v4();
    let username = "alice";
    sqlx::query("INSERT INTO users (id, username, totp_secret) VALUES (?1, ?2, NULL)")
        .bind(user_id.to_string())
        .bind(username)
        .execute(&pool)
        .await
        .expect("user row should be inserted");

    let auth_service = Arc::new(AuthServiceImpl::new(CryptoServiceImpl::with_defaults()));
    auth_service
        .create_user(username, SecretBox::new(Box::new(b"Admin1234!".to_vec())))
        .await
        .expect("auth user should exist for envelope-based key derivation");
    let totp_service = SqliteTotpService::new(
        pool.clone(),
        auth_service,
        CryptoServiceImpl::with_defaults(),
        "HeelonVault",
    );

    let payload = totp_service
        .create_setup_payload(username)
        .expect("setup payload should be created");
    let valid_code = build_current_totp_code(username, payload.base32_secret.as_str())
        .expect("current TOTP code should be generated");
    let wrong_code = mutate_totp_code(valid_code.as_str());

    let wrong_result = totp_service
        .enable_totp(
            user_id,
            username,
            payload.base32_secret.as_str(),
            wrong_code.as_str(),
        )
        .await;
    assert!(
        matches!(wrong_result, Err(AppError::Authorization(_))),
        "invalid TOTP code must be rejected before activation"
    );

    let row = sqlx::query("SELECT totp_secret FROM users WHERE id = ?1")
        .bind(user_id.to_string())
        .fetch_one(&pool)
        .await
        .expect("user row should exist");
    let stored_after_wrong: Option<Vec<u8>> =
        row.try_get("totp_secret").expect("column should exist");
    assert!(
        stored_after_wrong.is_none(),
        "totp_secret must stay NULL on invalid code"
    );

    let ok_result = totp_service
        .enable_totp(
            user_id,
            username,
            payload.base32_secret.as_str(),
            valid_code.as_str(),
        )
        .await;
    assert!(ok_result.is_ok(), "valid code should enable 2FA");

    let row = sqlx::query("SELECT totp_secret FROM users WHERE id = ?1")
        .bind(user_id.to_string())
        .fetch_one(&pool)
        .await
        .expect("user row should exist");
    let stored_after_ok: Option<Vec<u8>> = row.try_get("totp_secret").expect("column should exist");
    assert!(
        stored_after_ok.is_some(),
        "totp_secret should be stored once activation succeeds"
    );
}

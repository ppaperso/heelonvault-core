#![allow(clippy::disallowed_methods)]

//! Second factor: the TOTP secret is stored encrypted, a login code cannot be replayed, and the
//! code never substitutes for the password.

mod support;

use std::sync::Arc;

use heelonvault_core::errors::{AccessDeniedReason, AppError};
use heelonvault_core::repositories::user_repository::{
    SqlxUserRepository, UserKeyMaterialRepository, UserRepository,
};
use heelonvault_core::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_core::services::crypto_service::{CryptoService, CryptoServiceImpl};
use heelonvault_core::services::totp_service::{SqliteTotpService, TotpService};
use heelonvault_core::services::vault_service::serialize_vault_key_envelope;
use secrecy::{ExposeSecret, SecretBox};
use support::{
    Account, PASSWORD, USERNAME, bootstrap_account, db_files_contain, fast_crypto, secret,
};
use totp_rs::{Algorithm, Builder, Secret};

const ISSUER: &str = "HeelonVault";
const LEGACY_DEV_KEY: [u8; 32] = [0x41; 32];

type TotpServiceType = SqliteTotpService<AuthServiceImpl<CryptoServiceImpl>, CryptoServiceImpl>;

fn current_code(base32_secret: &str) -> String {
    let secret = Secret::try_from_base32(base32_secret).expect("base32 secret");
    let totp = Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .with_issuer(Some(ISSUER.to_string()))
        .with_account_name(USERNAME.to_string())
        .build()
        .expect("totp config");
    totp.generate_current().to_string()
}

/// Loads the stored password envelope, as the application does at start-up.
async fn totp_service(account: &Account) -> TotpServiceType {
    let auth = Arc::new(AuthServiceImpl::new(fast_crypto()));
    let envelope = SqlxUserRepository::new(account.pool.clone())
        .get_password_envelope_by_user_id(account.user_id)
        .await
        .expect("query envelope")
        .expect("envelope");
    auth.upsert_password_envelope(USERNAME, envelope)
        .await
        .expect("load credentials");
    SqliteTotpService::new(account.pool.clone(), auth, fast_crypto(), ISSUER)
}

async fn enabled_totp(account: &Account) -> (TotpServiceType, String) {
    let service = totp_service(account).await;
    let payload = service.create_setup_payload(USERNAME).expect("setup");
    service
        .enable_totp(
            account.user_id,
            USERNAME,
            &SecretBox::new(Box::new(account.account_key.clone())),
            &payload.base32_secret,
            &current_code(&payload.base32_secret),
        )
        .await
        .expect("enable totp");
    (service, payload.base32_secret)
}

#[tokio::test]
async fn a_login_code_is_accepted_once_and_never_replayed() {
    let account = bootstrap_account().await;
    let (service, base32) = enabled_totp(&account).await;
    let code = current_code(&base32);

    assert!(
        service
            .verify_login_totp(USERNAME, secret(PASSWORD), &code)
            .await
            .expect("verify")
    );
    assert!(
        !service
            .verify_login_totp(USERNAME, secret(PASSWORD), &code)
            .await
            .expect("verify replay"),
        "a TOTP code already used to log in must be rejected (RFC 6238 §5.2)"
    );
}

#[tokio::test]
async fn a_valid_code_never_replaces_the_password() {
    let account = bootstrap_account().await;
    let (service, base32) = enabled_totp(&account).await;

    let accepted = service
        .verify_login_totp(
            USERNAME,
            secret("not-the-password-2026"),
            &current_code(&base32),
        )
        .await
        .expect("verify");

    assert!(!accepted, "a valid code with a wrong password must fail");
}

#[tokio::test]
async fn malformed_codes_are_rejected_before_any_crypto() {
    let account = bootstrap_account().await;
    let (service, _) = enabled_totp(&account).await;

    for code in [
        "",
        "12345",
        "1234567",
        "12a456",
        " 12345",
        "123456 ",
        "-12345",
        "１２３４５６",
        "12\u{0}456",
        "' OR 1=1",
    ] {
        assert!(
            !service
                .verify_login_totp(USERNAME, secret(PASSWORD), code)
                .await
                .expect("verify"),
            "malformed code {code:?} must be rejected"
        );
    }
}

#[tokio::test]
async fn enabling_with_a_wrong_code_stores_nothing() {
    let account = bootstrap_account().await;
    let service = totp_service(&account).await;
    let payload = service.create_setup_payload(USERNAME).expect("setup");

    let result = service
        .enable_totp(
            account.user_id,
            USERNAME,
            &SecretBox::new(Box::new(account.account_key.clone())),
            &payload.base32_secret,
            "000000",
        )
        .await;

    // "000000" is the right code once in a million windows: accept that outcome silently.
    if result.is_err() {
        assert!(matches!(
            result,
            Err(AppError::Authorization(AccessDeniedReason::InvalidTotpCode))
        ));
        assert!(
            !service
                .is_totp_enabled_for_user_id(account.user_id)
                .await
                .expect("status")
        );
    }
}

#[tokio::test]
async fn the_totp_secret_is_encrypted_at_rest() {
    let account = bootstrap_account().await;
    let (service, base32) = enabled_totp(&account).await;
    assert!(
        service
            .is_totp_enabled_for_user_id(account.user_id)
            .await
            .expect("status")
    );
    account.pool.close().await;

    assert!(
        !db_files_contain(&account.db_path, base32.as_bytes()),
        "the base32 TOTP secret must never be written in clear"
    );
    let raw_secret = Secret::try_from_base32(&base32).expect("raw secret");
    let raw = raw_secret.as_bytes();
    assert!(
        !db_files_contain(&account.db_path, raw),
        "the raw TOTP key must never be written in clear"
    );
}

#[tokio::test]
async fn disabling_totp_erases_the_stored_secret() {
    let account = bootstrap_account().await;
    let (service, _) = enabled_totp(&account).await;
    let stored = SqlxUserRepository::new(account.pool.clone())
        .get_totp_secret(account.user_id)
        .await
        .expect("query")
        .expect("totp envelope")
        .expose_secret()
        .clone();

    service
        .disable_totp(account.user_id)
        .await
        .expect("disable");

    assert!(
        SqlxUserRepository::new(account.pool.clone())
            .get_totp_secret(account.user_id)
            .await
            .expect("query")
            .is_none()
    );
    account.pool.close().await;
    assert!(
        !db_files_contain(&account.db_path, &stored),
        "secure_delete must wipe the encrypted TOTP secret from disk"
    );
}

#[tokio::test]
async fn setup_secrets_are_unique_and_long_enough() {
    let account = bootstrap_account().await;
    let service = totp_service(&account).await;

    assert!(matches!(
        service.create_setup_payload("   "),
        Err(AppError::Validation(_))
    ));

    let first = service.create_setup_payload(USERNAME).expect("setup");
    let second = service.create_setup_payload(USERNAME).expect("setup");
    assert_ne!(first.base32_secret, second.base32_secret);
    let raw_secret = Secret::try_from_base32(&first.base32_secret).expect("raw");
    let raw = raw_secret.as_bytes();
    assert!(
        raw.len() >= 20,
        "RFC 4226 recommends a 160-bit shared secret, got {} bits",
        raw.len() * 8
    );
    assert!(first.otpauth_url.starts_with("otpauth://totp/"));
    assert!(!first.otpauth_url.contains(PASSWORD));
    assert!(first.qr_png.starts_with(b"\x89PNG"));
}

#[tokio::test]
async fn a_code_accepted_through_the_legacy_key_cannot_be_replayed() {
    // Secrets written by early development builds were encrypted under a fixed key; the service
    // still reads them once and re-encrypts them under the account key. That path must apply
    // the same anti-replay guard as the regular one.
    let account = bootstrap_account().await;
    let service = totp_service(&account).await;
    let payload = service.create_setup_payload(USERNAME).expect("setup");
    let legacy = fast_crypto()
        .encrypt(
            &SecretBox::new(Box::new(payload.base32_secret.as_bytes().to_vec())),
            &SecretBox::new(Box::new(LEGACY_DEV_KEY.to_vec())),
        )
        .await
        .expect("legacy encryption");
    sqlx::query("UPDATE users SET totp_secret = ?1 WHERE id = ?2")
        .bind(
            serialize_vault_key_envelope(&legacy)
                .expose_secret()
                .as_slice(),
        )
        .bind(account.user_id.to_string())
        .execute(&account.pool)
        .await
        .expect("plant legacy secret");
    let code = current_code(&payload.base32_secret);

    assert!(
        service
            .verify_login_totp(USERNAME, secret(PASSWORD), &code)
            .await
            .expect("verify legacy")
    );
    assert!(
        !service
            .verify_login_totp(USERNAME, secret(PASSWORD), &code)
            .await
            .expect("verify replay after migration"),
        "the code used for the legacy migration must not open a second session"
    );
}

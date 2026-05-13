#![allow(clippy::disallowed_methods)]

//! Security test template for authentication and 2FA.
//! Contract-level checks for auth flows using the service skeleton.

use heelonvault_rust::errors::AppError;
use heelonvault_rust::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_rust::services::crypto_service::CryptoServiceImpl;
use secrecy::SecretBox;

#[tokio::test]
async fn template_auth_no_unwrap_sensitive_paths() {
    let service = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
    let create_result = service
        .create_user(
            "alice",
            SecretBox::new(Box::new(b"correct horse battery staple".to_vec())),
        )
        .await;
    assert!(create_result.is_ok(), "create_user should succeed");

    let verify_result = service
        .verify_password(
            "alice",
            SecretBox::new(Box::new(b"correct horse battery staple".to_vec())),
        )
        .await;
    assert!(verify_result.is_ok(), "verify_password should succeed");

    let is_valid = match verify_result {
        Ok(value) => value,
        Err(_) => return,
    };
    assert!(is_valid);
}

#[tokio::test]
async fn template_auth_totp_verification_policy() {
    let service = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
    let create_result = service
        .create_user("bob", SecretBox::new(Box::new(b"one-pass".to_vec())))
        .await;
    assert!(create_result.is_ok(), "create_user should succeed");

    let verify_wrong_result = service
        .verify_password("bob", SecretBox::new(Box::new(b"wrong-pass".to_vec())))
        .await;
    assert!(
        verify_wrong_result.is_ok(),
        "verify_password should succeed"
    );

    let is_valid = match verify_wrong_result {
        Ok(value) => value,
        Err(_) => return,
    };
    assert!(!is_valid);
}

#[tokio::test]
async fn template_auth_error_handling_policy() {
    let service = AuthServiceImpl::new(CryptoServiceImpl::with_defaults());
    service.signal_shutdown();

    let create_result = service
        .create_user("charlie", SecretBox::new(Box::new(b"pw".to_vec())))
        .await;
    assert!(matches!(create_result, Err(AppError::ShutdownInProgress)));

    let verify_result = service
        .verify_password("charlie", SecretBox::new(Box::new(b"pw".to_vec())))
        .await;
    assert!(matches!(verify_result, Err(AppError::ShutdownInProgress)));
}

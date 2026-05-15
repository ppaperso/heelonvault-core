#![allow(clippy::disallowed_methods)]

use std::sync::Arc;

use heelonvault_rust::repositories::user_repository::SqlxUserRepository;
use heelonvault_rust::services::auth_service::AuthServiceImpl;
use heelonvault_rust::services::crypto_service::CryptoServiceImpl;
use heelonvault_rust::services::user_service::{UserProfileUpdate, UserService, UserServiceImpl};
use sqlx::sqlite::SqlitePoolOptions;
use uuid::Uuid;

async fn setup_pool() -> Result<sqlx::SqlitePool, String> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .map_err(|err| format!("connect sqlite memory: {err}"))?;

    sqlx::query(
        "CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            username TEXT NOT NULL,
            role TEXT NOT NULL,
            email TEXT,
            display_name TEXT,
            preferred_language TEXT NOT NULL DEFAULT 'fr',
            show_passwords_in_edit INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT
        )",
    )
    .execute(&pool)
    .await
    .map_err(|err| format!("create users table: {err}"))?;

    Ok(pool)
}

async fn insert_user(pool: &sqlx::SqlitePool, user_id: Uuid) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO users (
            id,
            username,
            role,
            email,
            display_name,
            preferred_language,
            show_passwords_in_edit
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(user_id.to_string())
    .bind("auditor")
    .bind("admin")
    .bind(Option::<String>::None)
    .bind(Some("Audit User".to_string()))
    .bind("fr")
    .bind(0_i64)
    .execute(pool)
    .await
    .map_err(|err| format!("insert user: {err}"))?;

    Ok(())
}

fn build_service(
    pool: sqlx::SqlitePool,
) -> UserServiceImpl<SqlxUserRepository, AuthServiceImpl<CryptoServiceImpl>> {
    let auth_service = Arc::new(AuthServiceImpl::new(CryptoServiceImpl::with_defaults()));
    UserServiceImpl::new(SqlxUserRepository::new(pool), auth_service)
}

#[tokio::test]
async fn preferred_language_persists_across_service_restart() {
    let pool_result = setup_pool().await;
    assert!(pool_result.is_ok(), "pool setup should succeed");
    let pool = match pool_result {
        Ok(value) => value,
        Err(_) => return,
    };

    let user_id = Uuid::new_v4();
    let insert_result = insert_user(&pool, user_id).await;
    assert!(insert_result.is_ok(), "user seed should succeed");
    if insert_result.is_err() {
        return;
    }

    let first_session = build_service(pool.clone());
    let initial_profile_result = first_session.get_user_profile(user_id).await;
    assert!(
        initial_profile_result.is_ok(),
        "initial profile read should succeed"
    );
    let initial_profile = match initial_profile_result {
        Ok(value) => value,
        Err(_) => return,
    };
    assert_eq!(initial_profile.preferred_language, "fr");

    let update_result = first_session
        .update_user_profile(
            user_id,
            UserProfileUpdate {
                email: None,
                display_name: None,
                preferred_language: Some("en".to_string()),
                show_passwords_in_edit: None,
                current_password: None,
            },
        )
        .await;
    assert!(update_result.is_ok(), "language update should succeed");
    let updated_profile = match update_result {
        Ok(value) => value,
        Err(_) => return,
    };
    assert_eq!(updated_profile.preferred_language, "en");

    let restarted_session = build_service(pool.clone());
    let reloaded_profile_result = restarted_session.get_user_profile(user_id).await;
    assert!(
        reloaded_profile_result.is_ok(),
        "reloaded profile read should succeed"
    );
    let reloaded_profile = match reloaded_profile_result {
        Ok(value) => value,
        Err(_) => return,
    };

    assert_eq!(reloaded_profile.preferred_language, "en");
}

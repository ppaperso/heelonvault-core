// `#[tokio::test]` expands to `Runtime::build().expect(..)`, which `disallowed_methods`
// reports here. Test code stays covered by `clippy::unwrap_used` / `clippy::expect_used`,
// enforced by `cargo clippy-secure` in CI.
#![allow(clippy::disallowed_methods)]

//! Tests de sécurité pour les injections SQL
//!
//! Validation que les repositories sont protégés contre les injections SQL
//! suite aux corrections des issues #35 et #37.

use anyhow::Context as _;
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use sqlx::sqlite::SqlitePoolOptions;
use uuid::Uuid;

/// Payloads malveillants classiques
const MALICIOUS_PAYLOADS: &[&str] = &[
    "'; DROP TABLE users; --",
    "'; DELETE FROM users WHERE '1'='1",
    "' UNION SELECT * FROM users --",
    "' OR '1'='1",
    "' OR 1=1 --",
    "'; --",
    "test'",
];

async fn setup_pool() -> anyhow::Result<sqlx::SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .context("Failed to connect")?;

    // Créer la table users avec le schema complet
    sqlx::query(
        "CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL,
            username TEXT NOT NULL UNIQUE,
            role TEXT NOT NULL,
            password_envelope BLOB,
            totp_secret_envelope BLOB,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_login TEXT,
            email TEXT,
            display_name TEXT,
            preferred_language TEXT,
            show_passwords_in_edit BOOLEAN DEFAULT FALSE,
            updated_at TEXT,
            CHECK (role IN ('user', 'admin'))
        )",
    )
    .execute(&pool)
    .await
    .context("Failed to create table")?;

    Ok(pool)
}

#[tokio::test]
async fn test_sql_injection_user_repository() -> anyhow::Result<()> {
    let pool = setup_pool().await?;
    let repo = SqlxUserRepository::new(pool.clone());

    // Créer un utilisateur valide
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, role) VALUES (?1, ?2, ?3)")
        .bind(user_id.to_string())
        .bind("valid_user")
        .bind("user")
        .execute(&pool)
        .await
        .context("Setup failed")?;

    // Tester que les payloads malveillants ne causent pas d'injection
    for payload in MALICIOUS_PAYLOADS {
        let user = UserRepository::get_by_username(&repo, payload)
            .await
            .with_context(|| format!("Query failed for payload: {payload}"))?;

        assert!(user.is_none(), "Injection successful for: {}", payload);
    }

    // Vérifier que l'utilisateur valide existe toujours
    let valid = UserRepository::get_by_username(&repo, "valid_user")
        .await
        .context("Failed to get valid user")?;
    assert!(valid.is_some());
    Ok(())
}

#[tokio::test]
async fn test_sql_injection_special_chars() -> anyhow::Result<()> {
    let pool = setup_pool().await?;
    let repo = SqlxUserRepository::new(pool.clone());

    let user_id = Uuid::new_v4();
    let special_name = "user'with\"quotes";

    sqlx::query("INSERT INTO users (id, username, role) VALUES (?1, ?2, ?3)")
        .bind(user_id.to_string())
        .bind(special_name)
        .bind("user")
        .execute(&pool)
        .await
        .context("Setup failed")?;

    let result = UserRepository::get_by_username(&repo, special_name)
        .await
        .context("Failed to get user")?;

    let user = result.context("the user with special characters must be found")?;
    assert_eq!(user.username, special_name);
    Ok(())
}

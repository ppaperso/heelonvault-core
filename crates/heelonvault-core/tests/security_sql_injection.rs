//! Tests de sécurité pour les injections SQL
//!
//! Validation que les repositories sont protégés contre les injections SQL
//! suite aux corrections des issues #35 et #37.

#![allow(clippy::disallowed_methods)]

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

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect");

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
    .expect("Failed to create table");

    pool
}

#[tokio::test]
async fn test_sql_injection_user_repository() {
    let pool = setup_pool().await;
    let repo = SqlxUserRepository::new(pool.clone());

    // Créer un utilisateur valide
    let user_id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, username, role) VALUES (?1, ?2, ?3)")
        .bind(user_id.to_string())
        .bind("valid_user")
        .bind("user")
        .execute(&pool)
        .await
        .expect("Setup failed");

    // Tester que les payloads malveillants ne causent pas d'injection
    for payload in MALICIOUS_PAYLOADS {
        let result = UserRepository::get_by_username(&repo, payload)
            .await;
        
        assert!(result.is_ok(), "Query failed for payload: {}", payload);
        assert!(result.unwrap().is_none(), "Injection successful for: {}", payload);
    }

    // Vérifier que l'utilisateur valide existe toujours
    let valid = UserRepository::get_by_username(&repo, "valid_user")
        .await
        .expect("Failed to get valid user");
    assert!(valid.is_some());
}

#[tokio::test]
async fn test_sql_injection_special_chars() {
    let pool = setup_pool().await;
    let repo = SqlxUserRepository::new(pool.clone());

    let user_id = Uuid::new_v4();
    let special_name = "user'with\"quotes";
    
    sqlx::query("INSERT INTO users (id, username, role) VALUES (?1, ?2, ?3)")
        .bind(user_id.to_string())
        .bind(special_name)
        .bind("user")
        .execute(&pool)
        .await
        .expect("Setup failed");

    let result = UserRepository::get_by_username(&repo, special_name)
        .await
        .expect("Failed to get user");
    
    assert!(result.is_some());
    assert_eq!(result.unwrap().username, special_name);
}

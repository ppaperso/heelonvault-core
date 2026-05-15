#![allow(clippy::disallowed_methods)]

use heelonvault_rust::services::login_history_service::{
    list_recent_logins, record_successful_login,
};
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
            password_envelope BLOB,
            totp_secret_envelope BLOB,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_login TEXT
        )",
    )
    .execute(&pool)
    .await
    .map_err(|err| format!("create users table: {err}"))?;

    sqlx::query(
        "CREATE TABLE login_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT NOT NULL,
            login_at TEXT NOT NULL,
            ip_address TEXT,
            device_info TEXT,
            FOREIGN KEY (user_id) REFERENCES users(id)
        )",
    )
    .execute(&pool)
    .await
    .map_err(|err| format!("create login_history table: {err}"))?;

    Ok(pool)
}

async fn insert_user(pool: &sqlx::SqlitePool, user_id: Uuid) -> Result<(), String> {
    sqlx::query("INSERT INTO users (id, username, role) VALUES (?1, ?2, ?3)")
        .bind(user_id.to_string())
        .bind("patrick")
        .bind("admin")
        .execute(pool)
        .await
        .map_err(|err| format!("insert user: {err}"))?;

    Ok(())
}

#[tokio::test]
async fn login_history_recorded_on_success_and_listed() {
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

    let record_result = record_successful_login(
        &pool,
        user_id,
        Some("127.0.0.1"),
        Some("Linux / GTK4 Desktop"),
    )
    .await;
    assert!(record_result.is_ok(), "login history insert should succeed");
    if record_result.is_err() {
        return;
    }

    let list_result = list_recent_logins(&pool, user_id, 5).await;
    assert!(list_result.is_ok(), "list recent logins should succeed");
    let items = match list_result {
        Ok(value) => value,
        Err(_) => return,
    };

    assert_eq!(items.len(), 1, "one login should be available in history");
    assert_eq!(items[0].ip_address.as_deref(), Some("127.0.0.1"));
    assert_eq!(
        items[0].device_info.as_deref(),
        Some("Linux / GTK4 Desktop")
    );
    assert!(
        !items[0].login_at.trim().is_empty(),
        "login_at should be present for UI popover display"
    );
}

#[tokio::test]
async fn login_history_recent_limit_and_order_are_respected() {
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

    let user_id_str = user_id.to_string();
    let seed_rows = [
        ("2026-03-18 08:00:00", "Linux / Device A"),
        ("2026-03-18 08:10:00", "Linux / Device B"),
        ("2026-03-18 08:20:00", "Linux / Device C"),
    ];
    for (login_at, device) in seed_rows {
        let seed_result = sqlx::query(
            "INSERT INTO login_history (user_id, login_at, ip_address, device_info)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(user_id_str.as_str())
        .bind(login_at)
        .bind("10.0.0.1")
        .bind(device)
        .execute(&pool)
        .await;
        assert!(seed_result.is_ok(), "seed login row should succeed");
        if seed_result.is_err() {
            return;
        }
    }

    let list_result = list_recent_logins(&pool, user_id, 2).await;
    assert!(list_result.is_ok(), "list recent logins should succeed");
    let items = match list_result {
        Ok(value) => value,
        Err(_) => return,
    };

    assert_eq!(items.len(), 2, "limit should cap history length to 2");
    assert_eq!(items[0].device_info.as_deref(), Some("Linux / Device C"));
    assert_eq!(items[1].device_info.as_deref(), Some("Linux / Device B"));
}

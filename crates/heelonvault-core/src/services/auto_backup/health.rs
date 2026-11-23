//! Database health, checked before each automatic backup, and the restore test.

use std::fs;
use std::path::{Path, PathBuf};

use secrecy::SecretBox;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::errors::AppError;
use crate::utils::private_fs;

use super::container;

/// Result of the SQLite checks. Holds no data from the database, only counts and sizes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DatabaseHealth {
    /// `PRAGMA quick_check` returned `ok`.
    pub integrity_ok: bool,
    /// Problems reported by `quick_check` (0 when `integrity_ok`).
    pub integrity_issues: u32,
    /// Rows reported by `PRAGMA foreign_key_check`.
    pub foreign_key_violations: u32,
    /// Latest applied migration, when the database was migrated by the application.
    pub schema_version: Option<i64>,
    pub database_bytes: u64,
    /// Size of the `-wal` file not yet checkpointed (0 when absent).
    pub wal_bytes: u64,
    pub secret_count: u64,
    pub vault_count: u64,
    pub user_count: u64,
}

impl DatabaseHealth {
    /// Integrity and references are sound.
    pub fn is_healthy(&self) -> bool {
        self.integrity_ok && self.foreign_key_violations == 0
    }
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn wal_path(db_path: &Path) -> PathBuf {
    let mut name = db_path.as_os_str().to_owned();
    name.push("-wal");
    PathBuf::from(name)
}

async fn count(pool: &SqlitePool, table: &str) -> Result<u64, AppError> {
    let exists: Option<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .bind(table)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Ok(0);
    }
    // `table` is one of the fixed names below, never user input.
    let total: i64 =
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(pool)
            .await?;
    Ok(u64::try_from(total).unwrap_or(0))
}

/// Runs the SQLite checks on an open database.
pub async fn check_pool(pool: &SqlitePool) -> Result<DatabaseHealth, AppError> {
    let quick: Vec<String> = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_all(pool)
        .await?;
    let integrity_ok = quick.len() == 1 && quick[0] == "ok";
    let foreign_keys = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await?;
    let migrations: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    let schema_version = match migrations {
        Some(_) => {
            sqlx::query("SELECT max(version) AS version FROM _sqlx_migrations WHERE success = 1")
                .fetch_one(pool)
                .await?
                .try_get::<Option<i64>, _>("version")?
        }
        None => None,
    };
    Ok(DatabaseHealth {
        integrity_ok,
        integrity_issues: if integrity_ok {
            0
        } else {
            u32::try_from(quick.len()).unwrap_or(u32::MAX)
        },
        foreign_key_violations: u32::try_from(foreign_keys.len()).unwrap_or(u32::MAX),
        schema_version,
        database_bytes: 0,
        wal_bytes: 0,
        secret_count: count(pool, "secret_items").await?,
        vault_count: count(pool, "vaults").await?,
        user_count: count(pool, "users").await?,
    })
}

/// Checks the live database and adds its file sizes.
pub async fn check_database(pool: &SqlitePool, db_path: &Path) -> Result<DatabaseHealth, AppError> {
    let mut health = check_pool(pool).await?;
    health.database_bytes = file_len(db_path);
    health.wal_bytes = file_len(&wal_path(db_path));
    Ok(health)
}

/// Removes the decrypted copy of a restore test, whatever happens.
struct PlaintextGuard(PathBuf);

impl Drop for PlaintextGuard {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut name = self.0.as_os_str().to_owned();
            name.push(suffix);
            let _ = fs::remove_file(PathBuf::from(name));
        }
    }
}

/// Restore test: decrypts a backup with the account key into an owner-only temporary file
/// in `work_dir`, opens it read-only and runs the health checks. The live database is never
/// touched and the decrypted copy is deleted before returning.
pub async fn test_restore(
    backup_path: &Path,
    account_key: &SecretBox<Vec<u8>>,
    work_dir: &Path,
) -> Result<DatabaseHealth, AppError> {
    let bytes = fs::read(backup_path).map_err(AppError::Io)?;
    let plaintext = container::open_with_account_key(&bytes, account_key)?;
    private_fs::create_private_dir_all(work_dir).map_err(AppError::Io)?;
    let temp = work_dir.join(format!(".restore-test-{}.db", uuid::Uuid::new_v4()));
    let guard = PlaintextGuard(temp.clone());
    private_fs::write_private(&temp, &plaintext).map_err(AppError::Io)?;
    drop(plaintext);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&temp)
                .read_only(true)
                .create_if_missing(false),
        )
        .await?;
    let result = check_pool(&pool).await;
    pool.close().await;
    let mut health = result?;
    health.database_bytes = file_len(&temp);
    drop(guard);
    Ok(health)
}

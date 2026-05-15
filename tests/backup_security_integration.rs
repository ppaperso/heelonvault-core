#![allow(clippy::disallowed_methods)]

/// Integration tests for backup security via BackupApplicationService
/// Tests authorization enforcement and data integrity
#[path = "common/mod.rs"]
mod common;

use common::{StubBackupService, StubUserRepo};
use heelonvault_rust::errors::AppError;
use heelonvault_rust::models::UserRole;
use heelonvault_rust::services::backup_application_service::BackupApplicationService;
use tempfile::TempDir;
use uuid::Uuid;
#[tokio::test]
async fn test_non_admin_cannot_export_backup() {
    // Case 1: Non-admin user attempts export
    let user_repo = StubUserRepo::default();
    let non_admin_id = Uuid::new_v4();
    user_repo.add_user(non_admin_id, UserRole::User);

    let backup_service = StubBackupService;
    let app_service =
        heelonvault_rust::services::backup_application_service::BackupApplicationServiceImpl::new(
            user_repo,
            backup_service,
        );

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let backup_path = temp_dir.path().join("backup.hvb");

    let result = app_service
        .export_backup_secured(
            non_admin_id,
            db_path.as_path(),
            backup_path.as_path(),
            &secrecy::SecretString::new("recovery phrase".to_string().into()),
        )
        .await;

    assert!(
        matches!(result, Err(AppError::Authorization(_))),
        "Expected Authorization error, but got: {:?}",
        result
    );
}

#[tokio::test]
async fn test_admin_can_export_backup() {
    // Case 2: Admin user can export
    let user_repo = StubUserRepo::default();
    let admin_id = Uuid::new_v4();
    user_repo.add_user(admin_id, UserRole::Admin);

    let backup_service = StubBackupService;
    let app_service =
        heelonvault_rust::services::backup_application_service::BackupApplicationServiceImpl::new(
            user_repo,
            backup_service,
        );

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let backup_path = temp_dir.path().join("backup.hvb");

    // Create dummy database file
    std::fs::write(&db_path, b"dummy db content").unwrap();

    let result = app_service
        .export_backup_secured(
            admin_id,
            db_path.as_path(),
            backup_path.as_path(),
            &secrecy::SecretString::new("recovery phrase".to_string().into()),
        )
        .await;

    assert!(
        result.is_ok(),
        "Admin export should succeed, but got error: {:?}",
        result
    );
}

#[tokio::test]
async fn test_missing_user_returns_not_found() {
    // Case: User that doesn't exist should return NotFound
    let user_repo = StubUserRepo::default();
    let missing_id = Uuid::new_v4();

    let backup_service = StubBackupService;
    let app_service =
        heelonvault_rust::services::backup_application_service::BackupApplicationServiceImpl::new(
            user_repo,
            backup_service,
        );

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let backup_path = temp_dir.path().join("backup.hvb");

    let result = app_service
        .export_backup_secured(
            missing_id,
            db_path.as_path(),
            backup_path.as_path(),
            &secrecy::SecretString::new("recovery phrase".to_string().into()),
        )
        .await;

    assert!(
        matches!(result, Err(AppError::NotFound(_))),
        "Expected NotFound error for missing user, but got: {:?}",
        result
    );
}

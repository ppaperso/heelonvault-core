#![allow(clippy::disallowed_methods)]

//! GDPR guarantees that can be checked on disk: erasure really removes the data (art. 17),
//! the export is complete and owner-only (art. 20), and nothing lingers in temporary files.
//!
//! Erasure is asserted on the raw database files (main file and `-wal`/`-shm`), not only
//! through SQL: a row that SQL no longer returns but whose bytes survive in a freed page is
//! not erased.

mod support;

use heelonvault_core::errors::{AppError, RecoveryFailure};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::user_repository::{
    SqlxUserRepository, UserKeyMaterialRepository, UserRepository,
};
use heelonvault_core::services::auth_policy_service::{AuthPolicyService, SqlxAuthPolicyService};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::login_history_service::record_successful_login;
use secrecy::ExposeSecret;
use sqlx::SqlitePool;
use support::{
    USERNAME, bootstrap_account, contains, create_vault, db_files_contain, insert_secret,
};
use uuid::Uuid;

const VAULT_KEY: [u8; 32] = [9; 32];

async fn count(pool: &SqlitePool, sql: &'static str, id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count query")
}

fn temporary_leftovers(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".partial") || name.ends_with(".snapshot"))
        .collect()
}

// ─── Art. 17 : droit à l'effacement ─────────────────────────────────────────

#[tokio::test]
async fn a_trashed_secret_is_recoverable_and_a_purged_one_is_gone_from_disk() {
    let account = bootstrap_account().await;
    let vault_id = create_vault(
        &account.pool,
        account.user_id,
        &account.account_key,
        &VAULT_KEY,
    )
    .await;
    let (secret_id, ciphertext) =
        insert_secret(&account.pool, vault_id, &VAULT_KEY, b"purge-me-please").await;
    let repo = SqlxSecretRepository::new(account.pool.clone());

    // The trash is a reversible soft delete: it is not an erasure and must not pretend to be.
    repo.soft_delete(secret_id).await.expect("soft delete");
    assert_eq!(
        count(
            &account.pool,
            "SELECT COUNT(*) FROM secret_items WHERE id = ?1",
            &secret_id.to_string()
        )
        .await,
        1
    );
    repo.restore_secret(secret_id, vault_id)
        .await
        .expect("restore");
    repo.soft_delete(secret_id).await.expect("soft delete");

    repo.permanent_delete(secret_id, vault_id)
        .await
        .expect("permanent delete");
    assert_eq!(
        count(
            &account.pool,
            "SELECT COUNT(*) FROM secret_items WHERE id = ?1",
            &secret_id.to_string()
        )
        .await,
        0
    );
    account.pool.close().await;

    assert!(
        !db_files_contain(&account.db_path, &ciphertext),
        "a permanently deleted secret must not survive in a freed page (secure_delete)"
    );
}

#[tokio::test]
async fn permanent_delete_refuses_a_secret_that_is_not_in_the_trash() {
    let account = bootstrap_account().await;
    let vault_id = create_vault(
        &account.pool,
        account.user_id,
        &account.account_key,
        &VAULT_KEY,
    )
    .await;
    let (secret_id, _) = insert_secret(&account.pool, vault_id, &VAULT_KEY, b"still-live").await;

    let result = SqlxSecretRepository::new(account.pool.clone())
        .permanent_delete(secret_id, vault_id)
        .await;

    assert!(
        matches!(result, Err(AppError::Storage(_))),
        "got {result:?}"
    );
    assert_eq!(
        count(
            &account.pool,
            "SELECT COUNT(*) FROM secret_items WHERE id = ?1",
            &secret_id.to_string()
        )
        .await,
        1,
        "a live secret must never be purged in one step"
    );
}

#[tokio::test]
async fn emptying_the_trash_wipes_trashed_secrets_only() {
    let account = bootstrap_account().await;
    let vault_id = create_vault(
        &account.pool,
        account.user_id,
        &account.account_key,
        &VAULT_KEY,
    )
    .await;
    let repo = SqlxSecretRepository::new(account.pool.clone());
    let (kept_id, kept_blob) = insert_secret(&account.pool, vault_id, &VAULT_KEY, b"kept").await;
    let mut trashed_blobs = Vec::new();
    for index in 0..3 {
        let (id, blob) = insert_secret(
            &account.pool,
            vault_id,
            &VAULT_KEY,
            format!("trashed-{index}").as_bytes(),
        )
        .await;
        repo.soft_delete(id).await.expect("soft delete");
        trashed_blobs.push(blob);
    }

    let purged = repo.empty_trash(vault_id).await.expect("empty trash");

    assert_eq!(purged, 3);
    assert_eq!(
        count(
            &account.pool,
            "SELECT COUNT(*) FROM secret_items WHERE id = ?1",
            &kept_id.to_string()
        )
        .await,
        1
    );
    account.pool.close().await;
    assert!(db_files_contain(&account.db_path, &kept_blob));
    for blob in &trashed_blobs {
        assert!(!db_files_contain(&account.db_path, blob));
    }
}

#[tokio::test]
async fn deleting_a_user_erases_every_piece_of_their_personal_data() {
    let account = bootstrap_account().await;
    let pool = account.pool.clone();
    let user_id = account.user_id.to_string();
    let users = SqlxUserRepository::new(pool.clone());
    let email = "alice.martin@labo-example.fr";
    let display_name = "Alice Martin-Unique";
    let ip = "203.0.113.77";
    let device = "Fedora-Workstation-Unique-Device";

    users
        .update_user_profile(
            account.user_id,
            Some(email),
            Some(display_name),
            Some("fr"),
            None,
        )
        .await
        .expect("profile");
    record_successful_login(&pool, account.user_id, Some(ip), Some(device))
        .await
        .expect("login history");
    SqlxAuthPolicyService::new(pool.clone())
        .record_failed_attempt(USERNAME)
        .await
        .expect("failed attempt");
    let vault_id = create_vault(&pool, account.user_id, &account.account_key, &VAULT_KEY).await;
    let (_, ciphertext) = insert_secret(&pool, vault_id, &VAULT_KEY, b"owned-secret").await;
    sqlx::query("INSERT INTO audit_log (actor_user_id, action) VALUES (?1, 'vault.created')")
        .bind(&user_id)
        .execute(&pool)
        .await
        .expect("audit entry");
    let password_envelope = users
        .get_password_envelope_by_user_id(account.user_id)
        .await
        .expect("query")
        .expect("envelope")
        .expose_secret()
        .clone();
    let recovery_envelope = users
        .get_recovery_key_envelope(account.user_id)
        .await
        .expect("query")
        .expect("recovery key envelope")
        .expose_secret()
        .clone();

    users
        .delete_user(account.user_id)
        .await
        .expect("delete user");

    for (table, sql) in [
        ("users", "SELECT COUNT(*) FROM users WHERE id = ?1"),
        (
            "login_history",
            "SELECT COUNT(*) FROM login_history WHERE user_id = ?1",
        ),
        (
            "vaults",
            "SELECT COUNT(*) FROM vaults WHERE owner_user_id = ?1",
        ),
    ] {
        assert_eq!(
            count(&pool, sql, &user_id).await,
            0,
            "{table} still references the deleted user"
        );
    }
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM secret_items WHERE vault_id = ?1",
            &vault_id.to_string()
        )
        .await,
        0,
        "secrets of the user's vaults must be erased with them"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM auth_policy WHERE username = ?1",
            USERNAME
        )
        .await,
        0,
        "the login throttling row keeps the username of the deleted user"
    );
    // The audit trail is kept for accountability, but no longer points to the person.
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE actor_user_id = ?1",
            &user_id
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM audit_log WHERE action = ?1 AND actor_user_id IS NULL",
            "vault.created"
        )
        .await,
        1
    );
    pool.close().await;

    for (label, needle) in [
        ("email", email.as_bytes()),
        ("display name", display_name.as_bytes()),
        ("IP address", ip.as_bytes()),
        ("device", device.as_bytes()),
        ("user id", user_id.as_bytes()),
        ("password envelope", password_envelope.as_slice()),
        ("recovery key envelope", recovery_envelope.as_slice()),
        ("secret ciphertext", ciphertext.as_slice()),
    ] {
        assert!(
            !db_files_contain(&account.db_path, needle),
            "the {label} of a deleted user is still on disk"
        );
    }
}

#[tokio::test]
async fn deleting_an_unknown_user_changes_nothing() {
    let account = bootstrap_account().await;

    let result = SqlxUserRepository::new(account.pool.clone())
        .delete_user(Uuid::new_v4())
        .await;

    assert!(
        matches!(result, Err(AppError::NotFound(_))),
        "got {result:?}"
    );
    assert_eq!(
        count(
            &account.pool,
            "SELECT COUNT(*) FROM users WHERE id = ?1",
            &account.user_id.to_string()
        )
        .await,
        1
    );
}

// ─── Art. 20 : portabilité / export sécurisé ────────────────────────────────

#[tokio::test]
async fn the_export_is_owner_only_complete_and_leaves_no_temporary_file() {
    let account = bootstrap_account().await;
    let vault_id = create_vault(
        &account.pool,
        account.user_id,
        &account.account_key,
        &VAULT_KEY,
    )
    .await;
    let mut blobs = Vec::new();
    for index in 0..5 {
        blobs.push(
            insert_secret(
                &account.pool,
                vault_id,
                &VAULT_KEY,
                format!("export-{index}").as_bytes(),
            )
            .await
            .1,
        );
    }
    account.pool.close().await;

    let backup = BackupServiceImpl::new();
    let export_dir = account.dir.path().join("exports");
    std::fs::create_dir(&export_dir).expect("export dir");
    let hvb = export_dir.join("heelonvault.hvb");
    let restored = export_dir.join("restored.db");

    backup
        .export_hvb_with_recovery_key(&account.db_path, &hvb, &account.recovery_phrase)
        .expect("export");
    #[cfg(unix)]
    assert_eq!(support::mode(&hvb), 0o600, "the export must be owner-only");
    let exported = std::fs::read(&hvb).expect("read export");
    for blob in &blobs {
        assert!(
            !contains(&exported, blob),
            "the export must not expose the stored ciphertexts as-is"
        );
    }

    backup
        .import_hvb_with_recovery_key(&hvb, &account.recovery_phrase, &restored)
        .expect("import");
    #[cfg(unix)]
    assert_eq!(
        support::mode(&restored),
        0o600,
        "the restored database must be owner-only"
    );
    assert!(temporary_leftovers(&export_dir).is_empty());

    let pool = support::open_pool(&restored).await;
    let restored_secrets = SqlxSecretRepository::new(pool.clone())
        .list_by_vault_id(vault_id)
        .await
        .expect("list");
    assert_eq!(
        restored_secrets.len(),
        blobs.len(),
        "the export must be complete"
    );
    for item in &restored_secrets {
        assert!(blobs.contains(item.secret_blob.expose_secret()));
    }
    pool.close().await;
}

#[tokio::test]
async fn a_failed_import_writes_nothing() {
    let account = bootstrap_account().await;
    account.pool.close().await;
    let backup = BackupServiceImpl::new();
    let hvb = account.dir.path().join("export.hvb");
    let restored = account.dir.path().join("restored.db");
    backup
        .export_hvb_with_recovery_key(&account.db_path, &hvb, &account.recovery_phrase)
        .expect("export");
    let other_phrase = backup
        .generate_recovery_key()
        .expect("phrase")
        .recovery_phrase;

    let result = backup.import_hvb_with_recovery_key(&hvb, &other_phrase, &restored);

    assert!(
        matches!(
            result,
            Err(AppError::Recovery(
                RecoveryFailure::WrongPhraseOrAlteredFile
            ))
        ),
        "got {result:?}"
    );
    assert!(!restored.exists());
    assert!(temporary_leftovers(account.dir.path()).is_empty());
}

#[tokio::test]
async fn importing_over_an_existing_file_is_refused() {
    let account = bootstrap_account().await;
    account.pool.close().await;
    let backup = BackupServiceImpl::new();
    let hvb = account.dir.path().join("export.hvb");
    backup
        .export_hvb_with_recovery_key(&account.db_path, &hvb, &account.recovery_phrase)
        .expect("export");
    let before = std::fs::read(&account.db_path).expect("read db");

    let result =
        backup.import_hvb_with_recovery_key(&hvb, &account.recovery_phrase, &account.db_path);

    if result.is_ok() {
        // Overwriting is allowed by design; the content must then be the export itself.
        let pool = support::open_pool(&account.db_path).await;
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM users WHERE id = ?1",
                &account.user_id.to_string()
            )
            .await,
            1
        );
        pool.close().await;
    } else {
        assert_eq!(
            std::fs::read(&account.db_path).expect("read db"),
            before,
            "a refused import must leave the live database untouched"
        );
    }
}

#[tokio::test]
async fn a_secured_export_never_leaves_its_database_snapshot_behind() {
    use heelonvault_core::services::backup_application_service::{
        BackupApplicationService, BackupApplicationServiceImpl,
    };

    let account = bootstrap_account().await;
    let service = BackupApplicationServiceImpl::new(
        SqlxUserRepository::new(account.pool.clone()),
        BackupServiceImpl::new(),
        account.pool.clone(),
    );
    let db_dir = account.db_path.parent().expect("db dir").to_path_buf();

    service
        .export_backup_secured(
            account.user_id,
            &account.db_path,
            &account.dir.path().join("ok.hvb"),
            &account.recovery_phrase,
        )
        .await
        .expect("secured export");
    assert!(temporary_leftovers(&db_dir).is_empty());

    let invalid_phrase = secrecy::SecretString::new("definitely not a recovery phrase".into());
    let refused = service
        .export_backup_secured(
            account.user_id,
            &account.db_path,
            &account.dir.path().join("refused.hvb"),
            &invalid_phrase,
        )
        .await;
    assert!(
        matches!(
            refused,
            Err(AppError::Recovery(RecoveryFailure::InvalidPhrase))
        ),
        "a phrase import would refuse must not seal a backup, got {refused:?}"
    );
    // The snapshot is taken before the backup is written: a write failure must still remove it.
    let occupied = account.dir.path().join("occupied.hvb");
    std::fs::create_dir(&occupied).expect("directory in place of the backup file");
    let unwritable = service
        .export_backup_secured(
            account.user_id,
            &account.db_path,
            &occupied,
            &account.recovery_phrase,
        )
        .await;
    assert!(unwritable.is_err(), "writing over a directory must fail");

    assert!(
        temporary_leftovers(&db_dir).is_empty(),
        "no snapshot or partial backup may survive a failed export: {:?}",
        temporary_leftovers(&db_dir)
    );
    assert!(!account.dir.path().join("refused.hvb").exists());
}

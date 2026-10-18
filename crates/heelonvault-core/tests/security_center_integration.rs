mod support;

use std::sync::Arc;

use heelonvault_core::models::{SecretType, UserRole, VaultShareRole};
use heelonvault_core::repositories::secret_repository::{SecretRepository, SqlxSecretRepository};
use heelonvault_core::repositories::team_repository::SqlxTeamRepository;
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::{SqlxVaultRepository, VaultRepository};
use heelonvault_core::services::audit_log_service::NoOpAuditLogService;
use heelonvault_core::services::secret_service::{SecretService, SecretServiceImpl};
use heelonvault_core::services::security_center::{Finding, ScanControl, UrlStatus, analyze};
use heelonvault_core::services::vault_service::VaultServiceImpl;
use secrecy::SecretBox;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn repeated_password_batches_scale_without_per_item_queries() -> TestResult {
    tokio::runtime::Runtime::new()?.block_on(async {
        let account = support::bootstrap_account().await;
        let key = [7_u8; 32];
        let vault = support::create_vault(&account.pool, account.user_id, &account.account_key, &key).await;
        let (_, blob) = support::insert_secret(&account.pool, vault, &key, b"eW8!rK2#qL7$vN4@zT9").await;
        let secrets = SecretServiceImpl::new(
            SqlxSecretRepository::new(account.pool.clone()), support::fast_crypto(), Arc::new(NoOpAuditLogService),
        );
        let vaults = VaultServiceImpl::new(
            SqlxVaultRepository::new(account.pool.clone()), SqlxVaultRepository::new(account.pool.clone()),
            SqlxUserRepository::new(account.pool.clone()), SqlxTeamRepository::new(account.pool.clone()),
            Arc::new(NoOpAuditLogService), support::fast_crypto(),
        );
        for size in [100, 1_000, 10_000] {
            sqlx::query("DELETE FROM secret_items").execute(&account.pool).await?;
            sqlx::query(
                "WITH RECURSIVE numbers(n) AS (
                    SELECT 1 UNION ALL SELECT n + 1 FROM numbers WHERE n < ?1
                 ) INSERT INTO secret_items (id, vault_id, secret_type, title, secret_blob, blob_storage)
                 SELECT printf('00000000-0000-4000-8000-%012d', n), ?2, 'password',
                        printf('entry-%d', n), ?3, 'inline' FROM numbers"
            ).bind(size).bind(vault.to_string()).bind(&blob).execute(&account.pool).await?;
            let started = std::time::Instant::now();
            let report = analyze(&secrets, &vaults, account.user_id,
                SecretBox::new(Box::new(account.account_key.clone())), OffsetDateTime::now_utc(),
                &ScanControl::default()).await?;
            let elapsed = started.elapsed();
            assert_eq!(report.analyzed, usize::try_from(size)?);
            assert_eq!(report.affected(), report.analyzed);
            assert!(report.is_complete());
            assert!(report.entries.iter().all(|entry| entry.reuse_group == Some(0)));
            assert!(elapsed < std::time::Duration::from_secs(30), "{size} items exceeded the 30s regression budget");
            eprintln!("Local repeated-password analysis: {size} items in {elapsed:?}");
        }
        Ok(())
    })
}

#[test]
fn batch_scan_reports_reuse_expiry_invalid_data_and_type_boundaries() -> TestResult {
    tokio::runtime::Runtime::new()?.block_on(async {
        let account = support::bootstrap_account().await;
        let key = [7_u8; 32];
        let first =
            support::create_vault(&account.pool, account.user_id, &account.account_key, &key).await;
        let second =
            support::create_vault(&account.pool, account.user_id, &account.account_key, &key).await;
        let secrets = SecretServiceImpl::new(
            SqlxSecretRepository::new(account.pool.clone()),
            support::fast_crypto(),
            Arc::new(NoOpAuditLogService),
        );
        let vaults = VaultServiceImpl::new(
            SqlxVaultRepository::new(account.pool.clone()),
            SqlxVaultRepository::new(account.pool.clone()),
            SqlxUserRepository::new(account.pool.clone()),
            SqlxTeamRepository::new(account.pool.clone()),
            Arc::new(NoOpAuditLogService),
            support::fast_crypto(),
        );
        let shared_password = "Password123!";
        for (vault, kind) in [
            (first, SecretType::Password),
            (second, SecretType::Password),
            (second, SecretType::ApiToken),
        ] {
            secrets
                .create_secret(
                    vault,
                    kind,
                    Some("test item".to_string()),
                    Some(r#"{"url":"http://example.com"}"#.to_string()),
                    None,
                    Some("2026-10-18T08:00:00Z".to_string()),
                    support::secret(shared_password),
                    SecretBox::new(Box::new(key.to_vec())),
                )
                .await?;
        }
        let corrupt = secrets
            .create_secret(
                first,
                SecretType::Password,
                Some("corrupt".to_string()),
                Some("{".to_string()),
                None,
                Some("tomorrow".to_string()),
                support::secret("a-password"),
                SecretBox::new(Box::new(key.to_vec())),
            )
            .await?;
        SqlxSecretRepository::new(account.pool.clone())
            .update_secret_blob(corrupt.id, support::secret("broken ciphertext"))
            .await?;
        let control = ScanControl::default();
        let report = analyze(
            &secrets,
            &vaults,
            account.user_id,
            SecretBox::new(Box::new(account.account_key.clone())),
            OffsetDateTime::parse("2026-10-18T08:00:00Z", &Rfc3339)?,
            &control,
        )
        .await?;
        assert_eq!(report.analyzed, 4);
        assert_eq!(report.affected(), 4);
        assert_eq!(report.vaults_analyzed, 2);
        assert_eq!(report.unreadable, 1);
        assert!(!report.is_complete());
        assert_eq!(
            report
                .entries
                .iter()
                .filter(|entry| entry.findings.contains(&Finding::ReusedPassword))
                .count(),
            2
        );
        assert_eq!(
            report
                .entries
                .iter()
                .filter(|entry| entry.findings.contains(&Finding::WeakPassword))
                .count(),
            2
        );
        assert_eq!(
            report
                .entries
                .iter()
                .filter(|entry| entry.findings.contains(&Finding::Expired))
                .count(),
            3
        );
        assert_eq!(
            report
                .entries
                .iter()
                .filter(|entry| entry.findings.contains(&Finding::Url(UrlStatus::Http)))
                .count(),
            3
        );
        let invalid = report
            .entries
            .iter()
            .find(|entry| entry.secret_id == corrupt.id)
            .ok_or("missing corrupt item")?;
        assert!(invalid.findings.contains(&Finding::InvalidMetadata));
        assert!(invalid.findings.contains(&Finding::InvalidExpiry));
        assert!(invalid.findings.contains(&Finding::Unreadable));
        assert!(!format!("{report:?}").contains(shared_password));
        assert_eq!(
            control.processed.load(std::sync::atomic::Ordering::Acquire),
            4
        );
        Ok(())
    })
}

#[test]
fn revoked_shares_are_excluded_and_unopenable_vaults_are_explicit() -> TestResult {
    tokio::runtime::Runtime::new()?.block_on(async {
        let account = support::bootstrap_account().await;
        let key = [7_u8; 32];
        let vault =
            support::create_vault(&account.pool, account.user_id, &account.account_key, &key).await;
        support::insert_secret(&account.pool, vault, &key, b"Password123!").await;
        let collaborator = uuid::Uuid::new_v4();
        let collaborator_key = [9_u8; 32];
        SqlxUserRepository::new(account.pool.clone())
            .create_user_db(collaborator, "collaborator", &UserRole::User)
            .await?;
        let repo = SqlxVaultRepository::new(account.pool.clone());
        repo.insert_key_share(
            vault,
            collaborator,
            support::encrypt_with(&collaborator_key, &key).await,
            Some(account.user_id),
            None,
            VaultShareRole::Read,
        )
        .await?;
        let vaults = VaultServiceImpl::new(
            SqlxVaultRepository::new(account.pool.clone()),
            SqlxVaultRepository::new(account.pool.clone()),
            SqlxUserRepository::new(account.pool.clone()),
            SqlxTeamRepository::new(account.pool.clone()),
            Arc::new(NoOpAuditLogService),
            support::fast_crypto(),
        );
        let secrets = SecretServiceImpl::new(
            SqlxSecretRepository::new(account.pool.clone()),
            support::fast_crypto(),
            Arc::new(NoOpAuditLogService),
        );
        let now = OffsetDateTime::now_utc();
        let report = analyze(
            &secrets,
            &vaults,
            collaborator,
            SecretBox::new(Box::new(collaborator_key.to_vec())),
            now,
            &ScanControl::default(),
        )
        .await?;
        assert_eq!(report.analyzed, 1);
        repo.delete_key_share(vault, collaborator).await?;
        let revoked = analyze(
            &secrets,
            &vaults,
            collaborator,
            SecretBox::new(Box::new(collaborator_key.to_vec())),
            now,
            &ScanControl::default(),
        )
        .await?;
        assert_eq!(revoked.analyzed, 0);
        let wrong_key = analyze(
            &secrets,
            &vaults,
            account.user_id,
            SecretBox::new(Box::new(vec![0; 32])),
            now,
            &ScanControl::default(),
        )
        .await?;
        assert_eq!(wrong_key.unavailable_vaults, 1);
        assert!(!wrong_key.is_complete());
        let control = ScanControl::default();
        control.cancel();
        assert!(
            analyze(
                &secrets,
                &vaults,
                account.user_id,
                SecretBox::new(Box::new(account.account_key.clone())),
                now,
                &control
            )
            .await
            .is_err()
        );
        Ok(())
    })
}

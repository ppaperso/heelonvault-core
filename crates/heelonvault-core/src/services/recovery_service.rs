use secrecy::SecretString;
use sqlx::SqlitePool;
use tracing::info;

use crate::errors::{AppError, RecoveryFailure};
use crate::models::User;
use crate::repositories::secret_repository::SqlxSecretRepository;
use crate::repositories::user_repository::{
    SqlxUserRepository, UserKeyMaterialRepository, UserRepository,
};
use crate::repositories::vault_repository::SqlxVaultRepository;
use crate::services::account_key::open_with_recovery_phrase;
use crate::services::auth_policy_service::{AuthPolicyService, SqlxAuthPolicyService};
use crate::services::auth_service::{PasswordEnvelope, decode_password_envelope};
use crate::services::crypto_service::CryptoService;
use crate::services::rekey_service::{
    RekeyReport, RekeyRepositories, migrate_to_account_key, replace_password,
};

/// Sets a new password on a database restored from a `.hvb` backup, from its recovery phrase.
///
/// An account-key backup is reopened by the account key sealed with the phrase. A legacy backup
/// still holds its master key in clear: it is migrated to an account key on the way.
pub async fn reset_master_password_from_backup<C>(
    pool: &SqlitePool,
    crypto: &C,
    recovery_phrase: &SecretString,
    new_password: &SecretString,
) -> Result<RekeyReport, AppError>
where
    C: CryptoService + Send + Sync,
{
    let user_repo = SqlxUserRepository::new(pool.clone());
    let vault_repo = SqlxVaultRepository::new(pool.clone());
    let secret_repo = SqlxSecretRepository::new(pool.clone());

    let user = match <[User; 1]>::try_from(user_repo.list_all().await?) {
        Ok([user]) => user,
        Err(users) if users.is_empty() => {
            return Err(AppError::Recovery(RecoveryFailure::NoAccount));
        }
        Err(_) => return Err(AppError::Recovery(RecoveryFailure::MultipleAccounts)),
    };

    let stored = user_repo
        .get_password_envelope_by_user_id(user.id)
        .await?
        .ok_or_else(|| {
            AppError::Storage("the restored account has no password envelope".to_string())
        })?;
    let repos = RekeyRepositories {
        users: &user_repo,
        vaults: &vault_repo,
        envelopes: &vault_repo,
        secrets: &secret_repo,
    };

    let outcome = match decode_password_envelope(&stored)? {
        PasswordEnvelope::Legacy { master_key, .. } => {
            migrate_to_account_key(
                repos,
                crypto,
                user.id,
                &master_key,
                new_password,
                Some(recovery_phrase),
            )
            .await?
        }
        PasswordEnvelope::AccountKey { .. } => {
            let sealed = user_repo
                .get_recovery_key_envelope(user.id)
                .await?
                .ok_or(AppError::Recovery(RecoveryFailure::RecoveryKeyMissing))?;
            // The phrase already decrypted the backup: a mismatch here means inconsistent data.
            let account_key = open_with_recovery_phrase(crypto, recovery_phrase, &sealed)
                .await
                .map_err(|_| AppError::Recovery(RecoveryFailure::InconsistentKeyMaterial))?;
            replace_password(repos, crypto, user.id, &account_key, new_password).await?
        }
    };

    SqlxAuthPolicyService::new(pool.clone())
        .reset_failed_attempts(&user.username)
        .await?;

    let report = outcome.report;
    info!(
        user_id = %user.id,
        owner_vaults_rewrapped = report.owner_vaults_rewrapped,
        vaults_without_key = report.vaults_without_key,
        secrets_readable = report.secrets_readable,
        secrets_unreadable = report.secrets_unreadable,
        "recovery reset: new password set on restored database"
    );
    Ok(report)
}

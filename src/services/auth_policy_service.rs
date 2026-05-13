use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tracing::{error, info};

use crate::errors::AppError;

const MAX_FAILED_ATTEMPTS: i64 = 5;
const LOCK_WINDOW_SECS: i64 = 5 * 60;
const BACKOFF_MAX_SECS: i64 = 30;
const DEFAULT_AUTO_LOCK_DELAY_MINS: i64 = 5;

#[derive(Debug, Clone, Copy)]
pub struct AuthPolicyState {
    pub failed_attempts: i64,
    pub remaining_lock_secs: i64,
}

impl AuthPolicyState {
    pub fn is_locked(&self) -> bool {
        self.failed_attempts >= MAX_FAILED_ATTEMPTS && self.remaining_lock_secs > 0
    }
}

#[trait_variant::make(AuthPolicyService: Send)]
pub trait LocalAuthPolicyService {
    async fn get_state(&self, username: &str) -> Result<AuthPolicyState, AppError>;
    async fn record_failed_attempt(&self, username: &str) -> Result<AuthPolicyState, AppError>;
    async fn reset_failed_attempts(&self, username: &str) -> Result<(), AppError>;
    async fn get_auto_lock_delay(&self, username: &str) -> Result<i64, AppError>;
    async fn update_auto_lock_delay(&self, username: &str, mins: i64) -> Result<(), AppError>;
}

pub struct SqlxAuthPolicyService {
    pool: SqlitePool,
}

impl SqlxAuthPolicyService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn ensure_row_exists(&self, username: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO auth_policy (username, failed_attempts, last_attempt_at)
             VALUES (?1, 0, NULL)
             ON CONFLICT(username) DO NOTHING",
        )
        .bind(username)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn backoff_delay_secs(failed_attempts: i64) -> i64 {
        if failed_attempts <= 0 {
            return 0;
        }

        let shift = (failed_attempts - 1).clamp(0, 30) as u32;
        let value = 1_i64.checked_shl(shift).unwrap_or(BACKOFF_MAX_SECS);
        value.min(BACKOFF_MAX_SECS)
    }

    fn remaining_lock_secs(failed_attempts: i64, last_attempt_at: Option<i64>, now_ts: i64) -> i64 {
        let Some(last_ts) = last_attempt_at else {
            return 0;
        };

        let elapsed = now_ts.saturating_sub(last_ts);
        let mut remaining = 0;

        if failed_attempts >= MAX_FAILED_ATTEMPTS && elapsed < LOCK_WINDOW_SECS {
            remaining = LOCK_WINDOW_SECS - elapsed;
        }

        let backoff = Self::backoff_delay_secs(failed_attempts);
        if backoff > elapsed {
            remaining = remaining.max(backoff - elapsed);
        }

        remaining
    }

    fn is_allowed_auto_lock_delay(mins: i64) -> bool {
        matches!(mins, 0 | 1 | 5 | 10 | 15 | 30)
    }
}

impl AuthPolicyService for SqlxAuthPolicyService {
    async fn get_state(&self, username: &str) -> Result<AuthPolicyState, AppError> {
        if username.trim().is_empty() {
            return Ok(AuthPolicyState {
                failed_attempts: 0,
                remaining_lock_secs: 0,
            });
        }

        self.ensure_row_exists(username).await?;

        let row = sqlx::query(
            "SELECT failed_attempts, last_attempt_at
             FROM auth_policy
             WHERE username = ?1",
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;

        let failed_attempts: i64 = row.try_get("failed_attempts")?;
        let last_attempt_at: Option<i64> = row.try_get("last_attempt_at")?;

        let now_ts = Utc::now().timestamp();
        Ok(AuthPolicyState {
            failed_attempts,
            remaining_lock_secs: Self::remaining_lock_secs(
                failed_attempts,
                last_attempt_at,
                now_ts,
            ),
        })
    }

    async fn record_failed_attempt(&self, username: &str) -> Result<AuthPolicyState, AppError> {
        if username.trim().is_empty() {
            return Ok(AuthPolicyState {
                failed_attempts: 1,
                remaining_lock_secs: 0,
            });
        }

        self.ensure_row_exists(username).await?;
        let now_ts = Utc::now().timestamp();

        sqlx::query(
            "UPDATE auth_policy
             SET failed_attempts = failed_attempts + 1,
                 last_attempt_at = ?2
             WHERE username = ?1",
        )
        .bind(username)
        .bind(now_ts)
        .execute(&self.pool)
        .await?;

        let state = AuthPolicyService::get_state(self, username).await?;
        if state.failed_attempts == 3 || state.failed_attempts == 5 {
            error!(
                username = %username,
                failed_attempts = state.failed_attempts,
                remaining_lock_secs = state.remaining_lock_secs,
                "critical login failure threshold reached"
            );
        }

        Ok(state)
    }

    async fn reset_failed_attempts(&self, username: &str) -> Result<(), AppError> {
        if username.trim().is_empty() {
            return Ok(());
        }

        self.ensure_row_exists(username).await?;

        let previous_failed_attempts: i64 =
            sqlx::query_scalar("SELECT failed_attempts FROM auth_policy WHERE username = ?1")
                .bind(username)
                .fetch_one(&self.pool)
                .await?;

        sqlx::query(
            "UPDATE auth_policy
             SET failed_attempts = 0,
                 last_attempt_at = NULL
             WHERE username = ?1",
        )
        .bind(username)
        .execute(&self.pool)
        .await?;

        info!(
            username = %username,
            previous_failed_attempts,
            "login success: failed attempts counter reset"
        );

        Ok(())
    }

    async fn get_auto_lock_delay(&self, username: &str) -> Result<i64, AppError> {
        if username.trim().is_empty() {
            return Ok(DEFAULT_AUTO_LOCK_DELAY_MINS);
        }

        self.ensure_row_exists(username).await?;

        let delay_opt: Option<i64> =
            sqlx::query_scalar("SELECT auto_lock_delay_mins FROM auth_policy WHERE username = ?1")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;

        let delay = delay_opt.unwrap_or(DEFAULT_AUTO_LOCK_DELAY_MINS);
        if Self::is_allowed_auto_lock_delay(delay) {
            Ok(delay)
        } else {
            Ok(DEFAULT_AUTO_LOCK_DELAY_MINS)
        }
    }

    async fn update_auto_lock_delay(&self, username: &str, mins: i64) -> Result<(), AppError> {
        if username.trim().is_empty() {
            return Err(AppError::Validation(
                "username must not be empty for auto-lock settings".to_string(),
            ));
        }
        if !Self::is_allowed_auto_lock_delay(mins) {
            return Err(AppError::Validation(
                "auto-lock delay must be one of: 0, 1, 5, 10, 15, 30".to_string(),
            ));
        }

        self.ensure_row_exists(username).await?;

        sqlx::query(
            "UPDATE auth_policy
             SET auto_lock_delay_mins = ?2
             WHERE username = ?1",
        )
        .bind(username)
        .bind(mins)
        .execute(&self.pool)
        .await?;

        info!(username = %username, auto_lock_delay_mins = mins, "auto-lock delay updated");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqlxAuthPolicyService;

    #[test]
    fn backoff_delay_is_exponential_and_capped() {
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(0), 0);
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(1), 1);
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(2), 2);
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(3), 4);
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(4), 8);
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(5), 16);
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(6), 30);
        assert_eq!(SqlxAuthPolicyService::backoff_delay_secs(16), 30);
    }

    #[test]
    fn remaining_lock_secs_applies_backoff_before_lock_threshold() {
        let remaining = SqlxAuthPolicyService::remaining_lock_secs(3, Some(100), 101);
        assert_eq!(remaining, 3);
    }

    #[test]
    fn remaining_lock_secs_keeps_hard_lock_priority() {
        let remaining = SqlxAuthPolicyService::remaining_lock_secs(5, Some(100), 110);
        assert_eq!(remaining, 290);
    }
}

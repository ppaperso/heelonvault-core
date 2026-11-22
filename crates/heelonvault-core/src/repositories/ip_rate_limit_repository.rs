//! Repository pour le rate limiting par IP.
//!
//! Ce module gère le stockage et la récupération des tentatives de connexion
//! par adresse IP pour implémenter un rate limiting et prévenir les attaques
//! par brute-force en essayant différents usernames.
//!
//! Issue: #41

use crate::errors::AppError;
use sqlx::{Row, SqlitePool};
use std::net::IpAddr;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Serialise un instant en RFC 3339 UTC à largeur fixe
/// (`YYYY-MM-DDTHH:MM:SS.nnnnnnnnn+00:00`).
///
/// `cleanup_expired` compare `locked_until` en SQL par ordre lexicographique :
/// une largeur fixe garantit que cet ordre suit l'ordre chronologique. Le suffixe
/// `+00:00` reste identique au format historiquement stocké par cette table.
fn format_timestamp(instant: OffsetDateTime) -> String {
    let utc = instant.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}+00:00",
        utc.year(),
        u8::from(utc.month()),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second(),
        utc.nanosecond()
    )
}

/// Politique de rate limiting par IP
#[derive(Clone, Debug)]
pub struct IpRateLimitPolicy {
    /// Nombre maximum de tentatives autorisées
    pub max_attempts: i32,
    /// Durée de lock après dépassement (en secondes)
    pub lock_duration_secs: i64,
    /// Période de fenêtre pour compter les tentatives (en secondes)
    pub window_duration_secs: i64,
}

impl Default for IpRateLimitPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 20,           // 20 tentatives max
            lock_duration_secs: 3600,   // 1 heure de lock
            window_duration_secs: 3600, // fenêtre de 1 heure
        }
    }
}

/// Enregistrement d'une tentative d'une IP
#[derive(Debug, Clone)]
pub struct IpLoginAttempt {
    pub ip: IpAddr,
    pub timestamp: OffsetDateTime,
}

/// Statut du rate limiting pour une IP
#[derive(Debug, Clone)]
pub struct IpRateLimitStatus {
    pub ip: IpAddr,
    pub attempts: i32,
    pub locked_until: Option<OffsetDateTime>,
}

impl IpRateLimitStatus {
    /// Vérifie si l'IP est actuellement bloquée
    pub fn is_locked(&self) -> bool {
        self.locked_until
            .map(|locked_until| locked_until > OffsetDateTime::now_utc())
            .unwrap_or(false)
    }

    /// Temps restant avant déblocage (en secondes), ou 0 si pas bloquée
    pub fn lock_remaining_seconds(&self) -> i64 {
        self.locked_until
            .map(|locked_until| {
                let now = OffsetDateTime::now_utc();
                if locked_until > now {
                    (locked_until - now).whole_seconds()
                } else {
                    0
                }
            })
            .unwrap_or(0)
    }
}

/// Trait pour le repository de rate limiting par IP
#[trait_variant::make(LocalIpRateLimitRepository: Send)]
pub trait IpRateLimitRepository {
    /// Enregistrer une tentative de connexion pour une IP
    fn record_attempt(
        &self,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<IpRateLimitStatus, AppError>> + Send;

    /// Vérifier le statut du rate limiting pour une IP
    fn check_rate_limit(
        &self,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<IpRateLimitStatus, AppError>> + Send;

    /// Réinitialiser les tentatives pour une IP (après succès de login)
    fn reset_attempts(
        &self,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<(), AppError>> + Send;

    /// Nettoyer les entrées expirées (à appeler périodiquement)
    fn cleanup_expired(&self) -> impl std::future::Future<Output = Result<u64, AppError>> + Send;
}

/// Implémentation SQLx du repository
pub struct SqlxIpRateLimitRepository {
    pool: SqlitePool,
    policy: IpRateLimitPolicy,
}

impl SqlxIpRateLimitRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            policy: IpRateLimitPolicy::default(),
        }
    }

    pub fn with_policy(pool: SqlitePool, policy: IpRateLimitPolicy) -> Self {
        Self { pool, policy }
    }
}

impl IpRateLimitRepository for SqlxIpRateLimitRepository {
    fn record_attempt(
        &self,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<IpRateLimitStatus, AppError>> + Send {
        let pool = &self.pool;
        let policy = self.policy.clone();
        async move {
            let ip_str = ip.to_string();
            let now = OffsetDateTime::now_utc();
            let window_start = now - Duration::from_secs(policy.window_duration_secs as u64);

            // Vérifier s'il y a une entrée existante
            let row_opt = sqlx::query(
                "SELECT ip, attempts, first_attempt_at, locked_until FROM login_attempts_ip WHERE ip = ?1",
            )
            .bind(&ip_str)
            .fetch_optional(pool)
            .await?;

            let (attempts, first_attempt_at, _existing_locked_until, has_existing) = match row_opt {
                Some(row) => {
                    let attempts: i32 = row.try_get("attempts")?;
                    let first_attempt_str: String = row.try_get("first_attempt_at")?;
                    let first_attempt_at = OffsetDateTime::parse(&first_attempt_str, &Rfc3339)
                        .map_err(|e| {
                            AppError::Storage(format!("invalid first_attempt_at format: {}", e))
                        })?;
                    let locked_until_opt: Option<String> = row.try_get("locked_until")?;
                    let _existing_locked_until = locked_until_opt
                        .map(|s| {
                            OffsetDateTime::parse(&s, &Rfc3339).map_err(|e| {
                                AppError::Storage(format!("invalid locked_until format: {}", e))
                            })
                        })
                        .transpose()?;

                    // Si déjà bloqué, retourner l'état actuel
                    if let Some(locked_until) = _existing_locked_until
                        && locked_until > now
                    {
                        return Ok(IpRateLimitStatus {
                            ip,
                            attempts,
                            locked_until: Some(locked_until),
                        });
                    }

                    // Réinitialiser si la fenêtre a expiré
                    if first_attempt_at < window_start {
                        (1, now, None::<OffsetDateTime>, true)
                    } else {
                        (attempts + 1, first_attempt_at, None::<OffsetDateTime>, true)
                    }
                }
                None => (1, now, None, false),
            };

            // Calculer si on doit bloquer
            let new_locked_until = if attempts >= policy.max_attempts {
                Some(now + Duration::from_secs(policy.lock_duration_secs as u64))
            } else {
                None
            };

            // Mettre à jour ou insérer
            if has_existing {
                sqlx::query(
                    "UPDATE login_attempts_ip SET attempts = ?1, first_attempt_at = ?2, locked_until = ?3 WHERE ip = ?4",
                )
                .bind(attempts)
                .bind(format_timestamp(first_attempt_at))
                .bind(new_locked_until.map(format_timestamp))
                .bind(&ip_str)
                .execute(pool)
                .await?;
            } else {
                sqlx::query(
                    "INSERT INTO login_attempts_ip (ip, attempts, first_attempt_at, locked_until) VALUES (?1, ?2, ?3, ?4)",
                )
                .bind(&ip_str)
                .bind(attempts)
                .bind(format_timestamp(first_attempt_at))
                .bind(new_locked_until.map(format_timestamp))
                .execute(pool)
                .await?;
            }

            Ok(IpRateLimitStatus {
                ip,
                attempts,
                locked_until: new_locked_until,
            })
        }
    }

    fn check_rate_limit(
        &self,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<IpRateLimitStatus, AppError>> + Send {
        let pool = &self.pool;
        async move {
            let ip_str = ip.to_string();

            let row_opt = sqlx::query(
                "SELECT ip, attempts, first_attempt_at, locked_until FROM login_attempts_ip WHERE ip = ?1",
            )
            .bind(&ip_str)
            .fetch_optional(pool)
            .await?;

            match row_opt {
                Some(row) => {
                    let attempts: i32 = row.try_get("attempts")?;
                    let locked_until_opt: Option<String> = row.try_get("locked_until")?;
                    let locked_until = locked_until_opt
                        .map(|s| {
                            OffsetDateTime::parse(&s, &Rfc3339).map_err(|e| {
                                AppError::Storage(format!("invalid locked_until format: {}", e))
                            })
                        })
                        .transpose()?;

                    Ok(IpRateLimitStatus {
                        ip,
                        attempts,
                        locked_until,
                    })
                }
                None => Ok(IpRateLimitStatus {
                    ip,
                    attempts: 0,
                    locked_until: None,
                }),
            }
        }
    }

    fn reset_attempts(
        &self,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<(), AppError>> + Send {
        let pool = &self.pool;
        async move {
            let ip_str = ip.to_string();
            let result = sqlx::query("DELETE FROM login_attempts_ip WHERE ip = ?1")
                .bind(&ip_str)
                .execute(pool)
                .await?;

            if result.rows_affected() == 0 {
                // Pas d'entrée à supprimer, ce n'est pas une erreur
                Ok(())
            } else {
                Ok(())
            }
        }
    }

    fn cleanup_expired(&self) -> impl std::future::Future<Output = Result<u64, AppError>> + Send {
        let pool = &self.pool;
        async move {
            let now = format_timestamp(OffsetDateTime::now_utc());
            let result = sqlx::query(
                "DELETE FROM login_attempts_ip WHERE locked_until IS NOT NULL AND locked_until < ?1",
            )
            .bind(&now)
            .execute(pool)
            .await?;

            Ok(result.rows_affected())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_timestamps_round_trip_and_sort_chronologically()
    -> Result<(), Box<dyn std::error::Error>> {
        let earlier = OffsetDateTime::from_unix_timestamp(1_774_000_000)?;
        let later = earlier + Duration::from_millis(1);
        let (a, b) = (format_timestamp(earlier), format_timestamp(later));
        assert_eq!(a, "2026-03-20T09:46:40.000000000+00:00");
        assert_eq!(a.len(), b.len());
        assert!(a < b, "lexicographic order must follow chronological order");
        assert_eq!(OffsetDateTime::parse(&a, &Rfc3339)?, earlier);
        assert_eq!(OffsetDateTime::parse(&b, &Rfc3339)?, later);
        Ok(())
    }

    #[test]
    fn non_utc_instants_are_stored_in_utc() -> Result<(), Box<dyn std::error::Error>> {
        let utc = OffsetDateTime::from_unix_timestamp(1_774_000_000)?;
        let paris = utc.to_offset(time::UtcOffset::from_hms(1, 0, 0)?);
        assert_eq!(format_timestamp(paris), format_timestamp(utc));
        Ok(())
    }

    #[test]
    fn legacy_chrono_rows_are_still_readable() -> Result<(), Box<dyn std::error::Error>> {
        // Shapes written by chrono's `to_rfc3339()` before the migration.
        let expected = OffsetDateTime::from_unix_timestamp(1_774_000_000)?;
        for legacy in [
            "2026-03-20T09:46:40+00:00",
            "2026-03-20T09:46:40.000123456+00:00",
        ] {
            let parsed = OffsetDateTime::parse(legacy, &Rfc3339)?;
            assert_eq!(parsed.unix_timestamp(), expected.unix_timestamp());
        }
        Ok(())
    }
}

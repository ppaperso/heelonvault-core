//! Repository pour le rate limiting par IP.
//!
//! Ce module gère le stockage et la récupération des tentatives de connexion
//! par adresse IP pour implémenter un rate limiting et prévenir les attaques
//! par brute-force en essayant différents usernames.
//!
//! Issue: #41

use crate::errors::AppError;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::net::IpAddr;
use std::time::Duration;

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
    pub timestamp: DateTime<Utc>,
}

/// Statut du rate limiting pour une IP
#[derive(Debug, Clone)]
pub struct IpRateLimitStatus {
    pub ip: IpAddr,
    pub attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
}

// SAFETY: DateTime<Utc> from chrono 0.4 is Send + Sync when all its components are Send + Sync.
// IpAddr is Send + Sync, i32 is Send + Sync, Option<DateTime<Utc>> needs explicit impl.
#[allow(unsafe_code)]
unsafe impl Send for IpRateLimitStatus {}
#[allow(unsafe_code)]
unsafe impl Sync for IpRateLimitStatus {}

impl IpRateLimitStatus {
    /// Vérifie si l'IP est actuellement bloquée
    pub fn is_locked(&self) -> bool {
        self.locked_until
            .map(|locked_until| locked_until > Utc::now())
            .unwrap_or(false)
    }

    /// Temps restant avant déblocage (en secondes), ou 0 si pas bloquée
    pub fn lock_remaining_seconds(&self) -> i64 {
        self.locked_until
            .map(|locked_until| {
                let now = Utc::now();
                if locked_until > now {
                    (locked_until - now).num_seconds()
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
            let now = Utc::now();
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
                    let first_attempt_at = DateTime::parse_from_rfc3339(&first_attempt_str)
                        .map_err(|e| {
                            AppError::Storage(format!("invalid first_attempt_at format: {}", e))
                        })?
                        .with_timezone(&Utc);
                    let locked_until_opt: Option<String> = row.try_get("locked_until")?;
                    let _existing_locked_until = locked_until_opt
                        .map(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .map_err(|e| {
                                    AppError::Storage(format!("invalid locked_until format: {}", e))
                                })
                                .map(|dt| dt.with_timezone(&Utc))
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
                        (1, now, None::<DateTime<Utc>>, true)
                    } else {
                        (attempts + 1, first_attempt_at, None::<DateTime<Utc>>, true)
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
                .bind(first_attempt_at.to_rfc3339())
                .bind(new_locked_until.map(|dt| dt.to_rfc3339()))
                .bind(&ip_str)
                .execute(pool)
                .await?;
            } else {
                sqlx::query(
                    "INSERT INTO login_attempts_ip (ip, attempts, first_attempt_at, locked_until) VALUES (?1, ?2, ?3, ?4)",
                )
                .bind(&ip_str)
                .bind(attempts)
                .bind(first_attempt_at.to_rfc3339())
                .bind(new_locked_until.map(|dt| dt.to_rfc3339()))
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
                            DateTime::parse_from_rfc3339(&s)
                                .map_err(|e| {
                                    AppError::Storage(format!("invalid locked_until format: {}", e))
                                })
                                .map(|dt| dt.with_timezone(&Utc))
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
            let now = Utc::now().to_rfc3339();
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

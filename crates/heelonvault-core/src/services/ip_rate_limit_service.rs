//! Service de rate limiting par IP pour la protection contre le brute-force.
//!
//! Ce service complémentaire au AuthPolicyService (qui gère le rate limiting par username)
//! permet de bloquer les attaques qui essaient différents usernames depuis la même IP.
//!
//! Issue: #41

use std::net::IpAddr;

use tracing::{error, info, warn};

use crate::errors::AppError;
use crate::repositories::ip_rate_limit_repository::{
    IpRateLimitPolicy, IpRateLimitRepository, IpRateLimitStatus,
};
use crate::services::auth_policy_service::{AuthPolicyService, AuthPolicyState};

/// Politique combinée pour le rate limiting
#[derive(Debug, Clone)]
pub struct CombinedRateLimitPolicy {
    /// Politique pour le rate limiting par username (existante)
    pub user_policy: AuthPolicyState,
    /// Politique pour le rate limiting par IP (nouvelle)
    pub ip_policy: IpRateLimitStatus,
}

impl CombinedRateLimitPolicy {
    /// Vérifie si l'utilisateur est bloqué (par username OU par IP)
    pub fn is_locked(&self) -> bool {
        self.user_policy.is_locked() || self.ip_policy.is_locked()
    }

    /// Temps restant avant déblocage (maximum des deux politiques)
    pub fn remaining_lock_seconds(&self) -> i64 {
        self.user_policy
            .remaining_lock_secs
            .max(self.ip_policy.lock_remaining_seconds())
    }

    /// Crée une nouvelle politique combinée
    pub fn new(user_policy: AuthPolicyState, ip_policy: IpRateLimitStatus) -> Self {
        Self {
            user_policy,
            ip_policy,
        }
    }
}

/// Trait pour le service combiné de rate limiting
#[trait_variant::make(CombinedRateLimitService: Send)]
pub trait LocalCombinedRateLimitService {
    /// Vérifier le statut du rate limiting pour un utilisateur depuis une IP
    fn check_combined_rate_limit(
        &self,
        username: &str,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<CombinedRateLimitPolicy, AppError>> + Send;

    /// Enregistrer une tentative échouée pour un utilisateur depuis une IP
    fn record_failed_attempt(
        &self,
        username: &str,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<CombinedRateLimitPolicy, AppError>> + Send;

    /// Réinitialiser les compteurs après un login réussi
    fn reset_on_success(
        &self,
        username: &str,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<(), AppError>> + Send;
}

/// Implémentation du service combiné
pub struct CombinedRateLimitServiceImpl<TUser, TIp> {
    user_service: TUser,
    ip_service: TIp,
}

impl<TUser, TIp> CombinedRateLimitServiceImpl<TUser, TIp>
where
    TUser: AuthPolicyService + Send + Sync,
    TIp: IpRateLimitRepository + Send + Sync,
{
    pub fn new(user_service: TUser, ip_service: TIp) -> Self {
        Self {
            user_service,
            ip_service,
        }
    }
}

impl<TUser, TIp> CombinedRateLimitService for CombinedRateLimitServiceImpl<TUser, TIp>
where
    TUser: AuthPolicyService + Send + Sync,
    TIp: IpRateLimitRepository + Send + Sync,
{
    fn check_combined_rate_limit(
        &self,
        username: &str,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<CombinedRateLimitPolicy, AppError>> + Send {
        let user_service = &self.user_service;
        let ip_service = &self.ip_service;
        async move {
            let user_policy = user_service.get_state(username).await?;
            let ip_policy = ip_service.check_rate_limit(ip).await?;

            Ok(CombinedRateLimitPolicy::new(user_policy, ip_policy))
        }
    }

    fn record_failed_attempt(
        &self,
        username: &str,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<CombinedRateLimitPolicy, AppError>> + Send {
        let user_service = &self.user_service;
        let ip_service = &self.ip_service;
        async move {
            // Enregistrer la tentative échouée pour l'utilisateur
            let user_policy = user_service.record_failed_attempt(username).await?;

            // Enregistrer la tentative échouée pour l'IP
            let ip_policy = ip_service.record_attempt(ip).await?;

            let combined = CombinedRateLimitPolicy::new(user_policy, ip_policy.clone());

            // Logging si l'IP est maintenant bloquée
            if ip_policy.is_locked() {
                warn!(
                    ip = %ip,
                    attempts = ip_policy.attempts,
                    lock_remaining = ip_policy.lock_remaining_seconds(),
                    "IP rate limit exceeded"
                );
            }

            // Logging si l'utilisateur est maintenant bloqué
            if user_policy.is_locked() {
                error!(
                    username = %username,
                    failed_attempts = user_policy.failed_attempts,
                    remaining_lock_secs = user_policy.remaining_lock_secs,
                    "user rate limit exceeded"
                );
            }

            Ok(combined)
        }
    }

    fn reset_on_success(
        &self,
        username: &str,
        ip: IpAddr,
    ) -> impl std::future::Future<Output = Result<(), AppError>> + Send {
        let user_service = &self.user_service;
        let ip_service = &self.ip_service;
        async move {
            // Réinitialiser les compteurs pour l'utilisateur
            user_service.reset_failed_attempts(username).await?;

            // Réinitialiser les compteurs pour l'IP
            ip_service.reset_attempts(ip).await?;

            info!(username = %username, ip = %ip, "rate limit counters reset on successful login");

            Ok(())
        }
    }
}

/// Service simplifié qui n'utilise que le rate limiting par IP
/// (utile si on n'a pas besoin du rate limiting par username)
#[allow(dead_code)]
pub struct SimpleIpRateLimitServiceImpl<TIp> {
    ip_service: TIp,
    policy: IpRateLimitPolicy,
}

impl<TIp> SimpleIpRateLimitServiceImpl<TIp>
where
    TIp: IpRateLimitRepository + Send + Sync,
{
    pub fn new(ip_service: TIp) -> Self {
        Self {
            ip_service,
            policy: IpRateLimitPolicy::default(),
        }
    }

    pub fn with_policy(ip_service: TIp, policy: IpRateLimitPolicy) -> Self {
        Self { ip_service, policy }
    }
}

impl<TIp> SimpleIpRateLimitServiceImpl<TIp>
where
    TIp: IpRateLimitRepository + Send + Sync,
{
    pub async fn check_ip_rate_limit(&self, ip: IpAddr) -> Result<IpRateLimitStatus, AppError> {
        self.ip_service.check_rate_limit(ip).await
    }

    pub async fn record_ip_attempt(&self, ip: IpAddr) -> Result<IpRateLimitStatus, AppError> {
        self.ip_service.record_attempt(ip).await
    }

    pub async fn reset_ip_attempts(&self, ip: IpAddr) -> Result<(), AppError> {
        self.ip_service.reset_attempts(ip).await
    }
}

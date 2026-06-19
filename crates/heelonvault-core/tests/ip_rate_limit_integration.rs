//! Tests d'intégration pour le rate limiting par IP
//!
//! Issue: #41 - Ajouter rate limiting IP-based pour le login

#![allow(clippy::disallowed_methods)]

use heelonvault_core::repositories::ip_rate_limit_repository::{
    IpRateLimitPolicy, IpRateLimitRepository, SqlxIpRateLimitRepository,
};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::{IpAddr, Ipv4Addr};

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect");

    // Créer la table pour le rate limiting IP
    sqlx::query(
        "CREATE TABLE login_attempts_ip (
            ip TEXT PRIMARY KEY NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            first_attempt_at TEXT NOT NULL,
            locked_until TEXT
        )",
    )
    .execute(&pool)
    .await
    .expect("Failed to create table");

    pool
}

#[tokio::test]
async fn test_ip_rate_limit_record_attempts() {
    let pool = setup_pool().await;
    let repo = SqlxIpRateLimitRepository::with_policy(
        pool,
        IpRateLimitPolicy {
            max_attempts: 5,          // 5 tentatives max pour le test
            lock_duration_secs: 60,   // 1 minute de lock
            window_duration_secs: 60, // fenêtre de 1 minute
        },
    );

    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    // 1ère tentative - doit réussir
    let status = repo
        .record_attempt(ip)
        .await
        .expect("Failed to record attempt");
    assert_eq!(status.attempts, 1);
    assert!(!status.is_locked());

    // 2ème tentative - doit réussir
    let status = repo
        .record_attempt(ip)
        .await
        .expect("Failed to record attempt");
    assert_eq!(status.attempts, 2);
    assert!(!status.is_locked());

    // Continuer jusqu'à 4
    for i in 3..=4 {
        let status = repo
            .record_attempt(ip)
            .await
            .expect("Failed to record attempt");
        assert_eq!(status.attempts, i);
        assert!(!status.is_locked());
    }

    // 5ème tentative - doit bloquer (5 >= 5)
    let status = repo
        .record_attempt(ip)
        .await
        .expect("Failed to record attempt");
    assert_eq!(status.attempts, 5);
    assert!(status.is_locked());
    assert!(status.lock_remaining_seconds() > 0);
}

#[tokio::test]
async fn test_ip_rate_limit_check_status() {
    let pool = setup_pool().await;
    let repo = SqlxIpRateLimitRepository::new(pool);

    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));

    // Nouvelle IP - doit avoir 0 tentative
    let status = repo
        .check_rate_limit(ip)
        .await
        .expect("Failed to check rate limit");
    assert_eq!(status.attempts, 0);
    assert!(!status.is_locked());
}

#[tokio::test]
async fn test_ip_rate_limit_reset() {
    let pool = setup_pool().await;
    let repo = SqlxIpRateLimitRepository::with_policy(
        pool,
        IpRateLimitPolicy {
            max_attempts: 3,
            lock_duration_secs: 60,
            window_duration_secs: 60,
        },
    );

    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 3));

    // Enregistrer des tentatives
    for _ in 0..3 {
        repo.record_attempt(ip)
            .await
            .expect("Failed to record attempt");
    }

    // Vérifier qu'il y a des tentatives enregistrées
    let status = repo
        .check_rate_limit(ip)
        .await
        .expect("Failed to check rate limit");
    assert_eq!(status.attempts, 3); // 3 appels = 3 tentatives

    // Réinitialiser
    repo.reset_attempts(ip).await.expect("Failed to reset");

    // Vérifier que c'est réinitialisé
    let status = repo
        .check_rate_limit(ip)
        .await
        .expect("Failed to check rate limit");
    assert_eq!(status.attempts, 0);
}

#[tokio::test]
async fn test_ip_rate_limit_multiple_ips() {
    let pool = setup_pool().await;
    let repo = SqlxIpRateLimitRepository::with_policy(
        pool,
        IpRateLimitPolicy {
            max_attempts: 2,
            lock_duration_secs: 60,
            window_duration_secs: 60,
        },
    );

    let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
    let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 11));

    // Enregistrer des tentatives pour IP1
    repo.record_attempt(ip1).await.expect("Failed");
    repo.record_attempt(ip1).await.expect("Failed");

    // IP1 doit être bloquée après 2 tentatives (max=2, commence à 1)
    let status1 = repo.record_attempt(ip1).await.expect("Failed");
    assert!(status1.is_locked());

    // IP2 ne doit pas être affectée
    let status2 = repo.check_rate_limit(ip2).await.expect("Failed");
    assert_eq!(status2.attempts, 0);
    assert!(!status2.is_locked());
}

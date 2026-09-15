#![allow(clippy::disallowed_methods)]

//! Security tests for brute-force resistance.
//! Verifies that security parameters sufficiently slow down attacks, and that the
//! IP rate-limiter actually locks out an attacker after too many failed attempts.

use heelonvault_core::errors::AppError;
use heelonvault_core::repositories::ip_rate_limit_repository::{
    IpRateLimitPolicy, IpRateLimitRepository, SqlxIpRateLimitRepository,
};
use heelonvault_core::services::account_key::generate_account_key;
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{CryptoService, CryptoServiceImpl, KdfConfig};
use heelonvault_core::services::password_service::{PasswordService, PasswordServiceImpl};
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use tempfile::TempDir;

async fn open_pool(path: &Path) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .pragma("secure_delete", "ON"),
        )
        .await
        .expect("open sqlite pool");
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../heelonvault-app/migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .expect("load migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    pool
}

fn password_bytes(value: &str) -> SecretBox<Vec<u8>> {
    SecretBox::new(Box::new(value.as_bytes().to_vec()))
}

// ============================================================================
// Catégorie 3 : Tests de résistance au brute-force
// ============================================================================

// ============================================================================
// Catégorie 3.1 : Tests des paramètres Argon2id
// ============================================================================

#[tokio::test]
async fn test_argon2id_parameters_are_sufficient() {
    // Une mesure d'horloge murale est intrinsèquement instable en CI (machine partagée,
    // charge variable) : on vérifie ici les *paramètres* de production plutôt qu'un temps
    // d'exécution mesuré, et on garde un test fonctionnel (dérivation réussie, bonne longueur).
    let config = KdfConfig::default();

    // Vérifier les paramètres par défaut : ce sont eux qui déterminent le coût réel
    // d'une attaque par brute-force hors-ligne, pas une mesure de temps en CI.
    assert_eq!(
        config.memory_cost_kib,
        64 * 1024,
        "Memory cost should be 64 MiB (65536 KiB), got {}",
        config.memory_cost_kib
    );
    assert_eq!(
        config.time_cost, 3,
        "Time cost should be 3, got {}",
        config.time_cost
    );
    assert_eq!(
        config.parallelism, 1,
        "Parallelism should be 1, got {}",
        config.parallelism
    );
    assert_eq!(
        config.output_len, 32,
        "Output length should be 32 bytes, got {}",
        config.output_len
    );

    let service = CryptoServiceImpl::new(config);
    let password = SecretString::new("test_password_1234567890".to_string().into());
    let salt = service
        .generate_kdf_salt()
        .await
        .expect("Salt generation failed");
    let key = service
        .derive_key(&password, &salt)
        .await
        .expect("Derivation failed");
    assert_eq!(
        key.expose_secret().len(),
        32,
        "Derived key should be 32 bytes"
    );
}

#[tokio::test]
async fn test_fast_argon2id_parameters() {
    // Test avec des paramètres réduits pour les tests
    let config = KdfConfig {
        memory_cost_kib: 1024, // 1 MiB
        time_cost: 1,
        parallelism: 1,
        output_len: 32,
    };

    let service = CryptoServiceImpl::new(config);
    let password = SecretString::new("test".to_string().into());
    let salt = service
        .generate_kdf_salt()
        .await
        .expect("Salt generation failed");

    let start = std::time::Instant::now();
    let _key = service
        .derive_key(&password, &salt)
        .await
        .expect("Derivation failed");
    let elapsed = start.elapsed();

    // Même avec des paramètres réduits, cela devrait prendre un certain temps
    println!("Fast Argon2id derivation time: {}ms", elapsed.as_millis());

    // Vérifier que la clé a la bonne longueur
    assert_eq!(_key.expose_secret().len(), 32, "Key should be 32 bytes");
}

#[tokio::test]
async fn test_different_salts_produce_different_keys() {
    let service = CryptoServiceImpl::with_defaults();
    let password = SecretString::new("same_password".to_string().into());

    let salt_1 = service.generate_kdf_salt().await.expect("generate salt");
    let salt_2 = service.generate_kdf_salt().await.expect("generate salt");

    // Les salts doivent être différents
    assert_ne!(
        salt_1.expose_secret(),
        salt_2.expose_secret(),
        "Salts should be unique"
    );

    let key_1 = service
        .derive_key(&password, &salt_1)
        .await
        .expect("derive key");
    let key_2 = service
        .derive_key(&password, &salt_2)
        .await
        .expect("derive key");

    // Les clés doivent être différentes
    assert_ne!(
        key_1.expose_secret(),
        key_2.expose_secret(),
        "Different salts should produce different keys"
    );
}

#[tokio::test]
async fn test_same_password_and_salt_produce_same_key() {
    let service = CryptoServiceImpl::with_defaults();
    let password = SecretString::new("test_password".to_string().into());
    let salt = service.generate_kdf_salt().await.expect("generate salt");

    let key_1 = service
        .derive_key(&password, &salt)
        .await
        .expect("derive key");
    let key_2 = service
        .derive_key(&password, &salt)
        .await
        .expect("derive key");

    // Les mêmes inputs doivent produire la même clé
    assert_eq!(
        key_1.expose_secret(),
        key_2.expose_secret(),
        "Same inputs should produce same key"
    );
}

#[tokio::test]
async fn test_salt_generation_is_random() {
    let service = CryptoServiceImpl::with_defaults();

    // Générer plusieurs salts et vérifier qu'ils sont tous différents
    let mut salts = Vec::new();
    for _ in 0..100 {
        let salt = service.generate_kdf_salt().await.expect("generate salt");
        salts.push(salt.expose_secret().clone());
    }

    // Vérifier que tous les salts sont uniques
    use std::collections::HashSet;
    let mut unique_salts = HashSet::new();
    for salt in &salts {
        assert!(
            unique_salts.insert(salt),
            "Duplicate salt generated! This is a critical security issue!"
        );
    }

    // Vérifier que chaque salt a la bonne longueur (32 octets = KDF_SALT_LEN)
    for salt in &salts {
        assert_eq!(
            salt.len(),
            32,
            "Salt should be 32 bytes, got {}",
            salt.len()
        );
    }
}

#[tokio::test]
async fn test_argon2id_with_different_passwords() {
    let service = CryptoServiceImpl::with_defaults();
    let salt = service.generate_kdf_salt().await.expect("generate salt");

    let password_1 = SecretString::new("password_1".to_string().into());
    let password_2 = SecretString::new("password_2".to_string().into());

    let key_1 = service
        .derive_key(&password_1, &salt)
        .await
        .expect("derive key");
    let key_2 = service
        .derive_key(&password_2, &salt)
        .await
        .expect("derive key");

    // Des mots de passe différents avec le même salt doivent produire des clés différentes
    assert_ne!(
        key_1.expose_secret(),
        key_2.expose_secret(),
        "Different passwords with same salt should produce different keys"
    );
}

// ============================================================================
// Catégorie 3.2 : Tests de complexité des secrets
// ============================================================================

#[tokio::test]
async fn test_recovery_phrase_entropy() {
    // Une phrase de récupération de 24 mots BIP39 a ~264 bits d'entropie
    // 2048 mots possibles par position
    // 24 mots → 2048^24 combinaisons

    // Vérifier que la génération de phrase produit bien 24 mots
    let phrase = BackupServiceImpl::new()
        .generate_recovery_key()
        .expect("Should generate phrase")
        .recovery_phrase;

    let word_count = phrase.expose_secret().split_whitespace().count();
    assert_eq!(
        word_count, 24,
        "Recovery phrase should have 24 words, got {}",
        word_count
    );

    // Vérifier que chaque mot est dans la liste BIP39 (crate bip39, déjà une dépendance
    // de heelonvault-core : c'est elle qui génère la phrase dans backup_service.rs)
    let bip39_words = bip39::Language::English.word_list();
    for word in phrase.expose_secret().split_whitespace() {
        assert!(
            bip39_words.contains(&word),
            "Word '{}' not in BIP39 wordlist",
            word
        );
    }

    // Vérifier que la phrase est en minuscules (normalisée)
    let phrase_lower = phrase.expose_secret().to_lowercase();
    assert_eq!(
        phrase.expose_secret(),
        &phrase_lower,
        "Recovery phrase should be lowercase"
    );
}

#[tokio::test]
async fn test_recovery_phrase_words_are_unique_in_position() {
    let phrase = BackupServiceImpl::new()
        .generate_recovery_key()
        .expect("Should generate phrase")
        .recovery_phrase;

    let words: Vec<&str> = phrase.expose_secret().split_whitespace().collect();
    assert_eq!(words.len(), 24, "Should have 24 words");

    // Chaque mot doit faire partie de la liste BIP39
    let bip39_words = bip39::Language::English.word_list();
    for word in &words {
        assert!(
            bip39_words.contains(word),
            "Word '{}' not in BIP39 wordlist",
            word
        );
    }
}

#[tokio::test]
async fn test_recovery_phrase_is_lowercase() {
    let phrase = BackupServiceImpl::new()
        .generate_recovery_key()
        .expect("Should generate phrase")
        .recovery_phrase;

    // Vérifier que la phrase est en minuscules
    let phrase_str = phrase.expose_secret();
    assert_eq!(
        phrase_str,
        &phrase_str.to_lowercase(),
        "Recovery phrase should be lowercase"
    );

    // Vérifier qu'il n'y a pas d'espaces doubles
    assert!(
        !phrase_str.contains("  "),
        "Recovery phrase should not contain double spaces"
    );
}

#[tokio::test]
async fn test_multiple_recovery_phrases_are_different() {
    // Générer plusieurs phrases et vérifier qu'elles sont différentes
    let mut phrases = Vec::new();
    for _ in 0..10 {
        let phrase = BackupServiceImpl::new()
            .generate_recovery_key()
            .expect("Should generate phrase")
            .recovery_phrase;
        phrases.push(phrase.expose_secret().to_string());
    }

    // Vérifier que toutes les phrases sont uniques
    use std::collections::HashSet;
    let mut unique_phrases = HashSet::new();
    for phrase in &phrases {
        assert!(
            unique_phrases.insert(phrase),
            "Duplicate recovery phrase generated!"
        );
    }
}

// ============================================================================
// Tests de politique de mot de passe
// ============================================================================

#[test]
fn test_password_policy_enforcement() {
    let policy = PasswordServiceImpl::new();

    // Mot de passe trop court (le minimum réel est 16 caractères, cf.
    // MIN_PASSWORD_LENGTH dans password_service.rs)
    let err = policy
        .validate_password_policy(&password_bytes("Short1!"))
        .expect_err("too-short password should be rejected");
    assert!(
        matches!(err, AppError::Validation(ref msg) if msg.contains("length")),
        "got {err:?}"
    );

    // Mot de passe avec espace (rejeté quelle que soit sa longueur)
    let err = policy
        .validate_password_policy(&password_bytes("Has A Space 123!"))
        .expect_err("password with whitespace should be rejected");
    assert!(
        matches!(err, AppError::Validation(ref msg) if msg.contains("whitespace")),
        "got {err:?}"
    );

    // Mot de passe sans majuscule (16+ caractères par ailleurs valides)
    let err = policy
        .validate_password_policy(&password_bytes("nouppercase1234!"))
        .expect_err("password without uppercase should be rejected");
    assert!(
        matches!(err, AppError::Validation(ref msg) if msg.contains("uppercase")),
        "got {err:?}"
    );

    // Mot de passe sans minuscule
    let err = policy
        .validate_password_policy(&password_bytes("NOLOWERCASE1234!"))
        .expect_err("password without lowercase should be rejected");
    assert!(
        matches!(err, AppError::Validation(ref msg) if msg.contains("lowercase")),
        "got {err:?}"
    );

    // Mot de passe sans chiffre
    let err = policy
        .validate_password_policy(&password_bytes("NoNumbersAtAll!!"))
        .expect_err("password without a digit should be rejected");
    assert!(
        matches!(err, AppError::Validation(ref msg) if msg.contains("digit")),
        "got {err:?}"
    );

    // Mot de passe sans symbole
    let err = policy
        .validate_password_policy(&password_bytes("NoSymbolsHere123"))
        .expect_err("password without a symbol should be rejected");
    assert!(
        matches!(err, AppError::Validation(ref msg) if msg.contains("symbol")),
        "got {err:?}"
    );

    // Mots de passe valides (>= 16 caractères, minuscule + majuscule + chiffre + symbole)
    assert!(
        policy
            .validate_password_policy(&password_bytes("ValidPassword123!"))
            .is_ok(),
        "Valid password should pass"
    );
    assert!(
        policy
            .validate_password_policy(&password_bytes("Another@ValidPass1"))
            .is_ok(),
        "Valid password should pass"
    );
    assert!(
        policy
            .validate_password_policy(&password_bytes("Test-Password#2026"))
            .is_ok(),
        "Valid password should pass"
    );
}

#[test]
fn test_password_policy_boundary_cases() {
    let policy = PasswordServiceImpl::new();
    // MIN_PASSWORD_LENGTH in password_service.rs (not exported, so mirrored here).
    const MIN_LEN: usize = 16;
    let base = "Ab1!";

    // Exactement à la longueur minimale : construit programmatiquement pour éviter
    // toute erreur de comptage manuel de caractères.
    let exactly_min = format!("{base}{}", "x".repeat(MIN_LEN - base.len()));
    assert_eq!(exactly_min.len(), MIN_LEN);
    assert!(
        policy
            .validate_password_policy(&password_bytes(&exactly_min))
            .is_ok(),
        "{MIN_LEN}-char password should be valid"
    );

    // Un caractère de moins que le minimum (trop court)
    let one_below_min = format!("{base}{}", "x".repeat(MIN_LEN - base.len() - 1));
    assert_eq!(one_below_min.len(), MIN_LEN - 1);
    let err = policy
        .validate_password_policy(&password_bytes(&one_below_min))
        .expect_err("one char below the minimum should be rejected");
    assert!(
        matches!(err, AppError::Validation(ref msg) if msg.contains("length")),
        "got {err:?}"
    );

    // Mot de passe avec tous les caractères spéciaux requis
    let special_chars = [
        "!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "-", "_", "+", "=",
    ];
    for special in special_chars {
        let password = format!("TestPassword123{special}");
        assert!(
            policy
                .validate_password_policy(&password_bytes(&password))
                .is_ok(),
            "Password with special char '{special}' should be valid"
        );
    }
}

#[test]
fn test_password_policy_accepts_unicode_alongside_ascii_requirements() {
    // La politique réelle (password_service.rs) classe les octets non-ASCII comme des
    // "symboles" : un mot de passe combinant des caractères Unicode avec les exigences
    // ASCII (majuscule/minuscule/chiffre) est donc accepté, pas rejeté.
    let policy = PasswordServiceImpl::new();
    let mixed = "TestПароль123!";
    assert!(
        mixed.len() >= 16,
        "fixture must satisfy the length policy for this assertion to be meaningful"
    );
    assert!(
        policy
            .validate_password_policy(&password_bytes(mixed))
            .is_ok(),
        "Unicode alongside ASCII upper/lower/digit should be accepted, not rejected"
    );

    // À l'inverse, une phrase entièrement non-ASCII échoue : elle ne contient aucune
    // majuscule/minuscule/chiffre ASCII, quelle que soit sa longueur.
    let pure_unicode = "пароль пароль пароль пароль";
    let err = policy
        .validate_password_policy(&password_bytes(pure_unicode))
        .expect_err("a password with no ASCII uppercase/lowercase/digit should be rejected");
    assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
}

// ============================================================================
// Tests de complexité PIN
// ============================================================================

#[test]
fn test_pin_complexity_requirements() {
    use heelonvault_core::services::pin_cache_service::{PIN_MAX_LEN, PIN_MIN_LEN, validate_pin};

    // PIN trop court
    assert!(
        validate_pin("123").is_err(),
        "PIN with {} digits should be rejected",
        PIN_MIN_LEN - 1
    );

    // PIN trop long
    assert!(
        validate_pin("123456789").is_err(),
        "PIN with {} digits should be rejected",
        PIN_MAX_LEN + 1
    );

    // PIN valide (4 chiffres)
    assert!(validate_pin("1234").is_ok(), "4-digit PIN should be valid");

    // PIN valide (8 chiffres)
    assert!(
        validate_pin("12345678").is_ok(),
        "8-digit PIN should be valid"
    );

    // PIN avec des caractères non-numériques
    assert!(
        validate_pin("12ab").is_err(),
        "PIN with non-digit characters should be rejected"
    );
    assert!(
        validate_pin("abcd").is_err(),
        "PIN with letters should be rejected"
    );
    assert!(
        validate_pin("12 34").is_err(),
        "PIN with spaces should be rejected"
    );
    assert!(
        validate_pin("12-34").is_err(),
        "PIN with dashes should be rejected"
    );
}

// ============================================================================
// Tests de timing (pour détecter les vulnérabilités de timing)
// ============================================================================

#[tokio::test]
async fn test_password_comparison_behaves_correctly_for_right_and_wrong_password() {
    // La comparaison à temps constant elle-même (subtle::ConstantTimeEq, utilisée dans
    // account_key.rs) est une garantie de code, pas quelque chose qu'on peut vérifier de
    // façon fiable en mesurant l'horloge murale d'un test (bruit CI, ordonnancement du
    // scheduler, etc. dominent largement un éventuel écart de calcul). On se contente donc
    // ici de vérifier le comportement fonctionnel : le bon mot de passe déverrouille, le
    // mauvais échoue, sans jamais paniquer.

    use heelonvault_core::services::auth_service::unlock_password_envelope;
    use heelonvault_core::services::crypto_service::CryptoServiceImpl;
    use secrecy::SecretString;

    let crypto = CryptoServiceImpl::with_defaults();
    let password = "correct_password_123";

    let account_key = generate_account_key().expect("Failed to generate account key");
    let password_envelope = heelonvault_core::services::auth_service::wrap_account_key(
        &crypto,
        &SecretString::new(password.to_string().into()),
        &account_key,
    )
    .await
    .expect("Failed to wrap account key");

    let encoded =
        heelonvault_core::services::auth_service::encode_password_envelope(&password_envelope);
    let decoded = heelonvault_core::services::auth_service::decode_password_envelope(&encoded)
        .expect("Failed to decode");

    let correct_result = unlock_password_envelope(
        &crypto,
        &decoded,
        &SecretString::new(password.to_string().into()),
    )
    .await
    .expect("Should not panic");
    assert!(
        correct_result.is_some(),
        "the correct password must unlock the envelope"
    );

    let wrong_result = unlock_password_envelope(
        &crypto,
        &decoded,
        &SecretString::new("wrong_password".to_string().into()),
    )
    .await
    .expect("Should not panic");
    assert!(
        wrong_result.is_none(),
        "the wrong password must not unlock the envelope"
    );
}

// ============================================================================
// Tests de rate limiting par IP (résistance réelle au brute-force)
// ============================================================================

#[tokio::test]
async fn test_ip_rate_limit_locks_after_max_attempts() {
    // Le nom de ce fichier promet une résistance au brute-force : ce test exerce le vrai
    // service de rate limiting par IP (table `login_attempts_ip`, migrée depuis les vraies
    // migrations SQLx) plutôt que de rester cantonné aux paramètres cryptographiques.
    let dir = TempDir::new().expect("temp dir");
    let pool = open_pool(&dir.path().join("ratelimit.db")).await;
    let attacker_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 42));

    let policy = IpRateLimitPolicy {
        max_attempts: 3,
        lock_duration_secs: 60,
        window_duration_secs: 60,
    };
    let repo = SqlxIpRateLimitRepository::with_policy(pool.clone(), policy);

    // Sous le seuil : l'IP n'est pas verrouillée.
    for attempt in 1..=2 {
        let status = repo
            .record_attempt(attacker_ip)
            .await
            .expect("record attempt");
        assert!(
            !status.is_locked(),
            "should not be locked after {attempt} attempt(s)"
        );
    }

    // Le 3e essai échoué atteint max_attempts : l'IP doit être verrouillée.
    let status = repo
        .record_attempt(attacker_ip)
        .await
        .expect("record attempt");
    assert!(
        status.is_locked(),
        "IP must be locked after reaching max_attempts"
    );
    assert!(
        status.lock_remaining_seconds() > 0,
        "a locked IP must report remaining lock time"
    );

    // check_rate_limit doit refléter le même état verrouillé pour un essai ultérieur.
    let checked = repo
        .check_rate_limit(attacker_ip)
        .await
        .expect("check rate limit");
    assert!(
        checked.is_locked(),
        "check_rate_limit must also report the IP as locked"
    );

    // reset_attempts (après un login réussi ailleurs, ou par un admin) lève le verrou.
    repo.reset_attempts(attacker_ip)
        .await
        .expect("reset attempts");
    let after_reset = repo
        .check_rate_limit(attacker_ip)
        .await
        .expect("check rate limit");
    assert!(
        !after_reset.is_locked(),
        "resetting attempts must clear the lock"
    );

    pool.close().await;
}

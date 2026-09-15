#![allow(clippy::disallowed_methods)]

//! Security tests for memory safety.
//!
//! # Garanties de zeroization du codebase HeelonVault
//!
//! - `Zeroizing<Vec<u8>>` : utilisé pour les salts et les buffers intermédiaires (KDF, AES).
//! - `SecretBox<T>` : utilisé pour toutes les données sensibles stockées (clés, blobs chiffrés).
//! - `SecretString` : utilisé pour les mots de passe et phrases de récupération.
//! - `PinCache` (`pin_cache_service.rs`) : implémente `Drop` avec zeroization de
//!   `encrypted_master_key`, `salt`, `nonce`, `failed_attempts`.
//!
//! ## Pourquoi ce fichier ne "teste" pas la mise à zéro mémoire elle-même
//!
//! En Rust sûr — et ce dépôt interdit tout bloc `unsafe` via `-Dunsafe_code`
//! (`.cargo/config.toml`) — il n'existe aucun moyen observable de vérifier après coup que
//! la mémoire libérée par un `Drop` a bien été mise à zéro : cela nécessiterait de relire une
//! zone mémoire déjà rendue au système, ce qui est justement ce que `unsafe` interdit d'exposer
//! sans un pointeur brut, ou de passer par un outil externe (Miri, valgrind). Un test qui se
//! contente de construire une valeur, vérifier une propriété fonctionnelle, puis `drop(x)` suivi
//! d'un commentaire "on ne peut pas vérifier directement" n'apporte aucune garantie au-delà de ce
//! que les tests fonctionnels du service concerné (crypto_service, account_key, ...) vérifient
//! déjà ailleurs — ces doublons ont été retirés d'ici.
//!
//! Ce fichier se limite donc à des propriétés réellement observables en Rust sûr :
//! - le format `Debug` de `SecretBox`/`SecretString` ne doit jamais exposer le secret en clair
//!   (garanti structurellement par la crate `secrecy`, revérifié ici) ;
//! - les messages d'erreur (`AppError`) ne doivent jamais contenir un secret en clair.
//!
//! Vérifier la mise à zéro mémoire elle-même resterait possible via `cargo miri test` en CI
//! (piste future, hors scope de ce fichier).

use heelonvault_core::errors::AppError;
use heelonvault_core::services::crypto_service::{CryptoService, CryptoServiceImpl, KdfConfig};
use secrecy::{ExposeSecret, SecretBox, SecretString};

fn fast_crypto() -> CryptoServiceImpl {
    CryptoServiceImpl::new(KdfConfig {
        memory_cost_kib: 1024,
        time_cost: 1,
        parallelism: 1,
        output_len: 32,
    })
}

#[test]
fn test_secret_box_debug_never_exposes_the_secret() {
    let secret = SecretBox::new(Box::new(b"my_super_secret_data_123!".to_vec()));

    let debug_output = format!("{secret:?}");

    assert!(
        debug_output.contains("REDACTED"),
        "SecretBox's Debug output should say REDACTED, got: {debug_output}"
    );
    assert!(
        !debug_output.contains("my_super_secret_data_123!"),
        "SecretBox's Debug output must never contain the plaintext secret, got: {debug_output}"
    );
}

#[test]
fn test_secret_string_debug_never_exposes_the_secret() {
    let secret = SecretString::new("my_super_secret_password".to_string().into());

    let debug_output = format!("{secret:?}");

    assert!(
        debug_output.contains("REDACTED"),
        "SecretString's Debug output should say REDACTED, got: {debug_output}"
    );
    assert!(
        !debug_output.contains("my_super_secret_password"),
        "SecretString's Debug output must never contain the plaintext secret, got: {debug_output}"
    );
    // The plaintext must still be reachable through the explicit API.
    assert_eq!(secret.expose_secret(), "my_super_secret_password");
}

#[tokio::test]
async fn test_no_sensitive_data_in_error_messages() {
    let service = fast_crypto();
    let password = SecretString::new("test".to_string().into());
    let empty_salt = SecretBox::new(Box::new(Vec::new()));

    // Cela devrait échouer car le salt est vide
    let result = service.derive_key(&password, &empty_salt).await;

    let err = result.expect_err("derive_key with an empty salt should fail");
    let error_string = format!("{err:?}");
    assert!(
        !error_string.contains("test"),
        "Error message should not contain the password: {error_string}"
    );

    if let AppError::Crypto(msg) = err {
        assert!(
            !msg.contains("test"),
            "Crypto error message should not contain the password: {msg}"
        );
    }
}

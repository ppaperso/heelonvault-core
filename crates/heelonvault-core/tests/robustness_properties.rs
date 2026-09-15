#![allow(clippy::disallowed_methods)]

//! Robustness under hostile input, checked with generated data rather than hand-picked cases:
//! every decoder of attacker-controllable bytes (database columns, backup files, PIN input)
//! returns an error instead of panicking, AES-GCM rejects any single-bit alteration, and large
//! payloads survive a round trip.

mod support;

use heelonvault_core::services::account_key::open_with_recovery_phrase;
use heelonvault_core::services::auth_service::{
    decode_password_envelope, unlock_password_envelope,
};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::{
    CryptoService, CryptoServiceImpl, ENVELOPE_VERSION, EncryptedPayload, NONCE_LEN,
    decode_envelope,
};
use heelonvault_core::services::password_service::{PasswordService, PasswordServiceImpl};
use heelonvault_core::services::pin_cache_service::PinCache;
use heelonvault_core::services::vault_service::deserialize_vault_key_envelope;
use proptest::prelude::*;
use secrecy::{ExposeSecret, SecretBox, SecretString};
use support::fast_crypto;
use uuid::Uuid;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio runtime")
}

fn boxed(bytes: &[u8]) -> SecretBox<Vec<u8>> {
    SecretBox::new(Box::new(bytes.to_vec()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn versioned_envelope_decoding_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let result = decode_envelope(&bytes);
        let well_formed = bytes.len() > 1 + NONCE_LEN && bytes[0] == ENVELOPE_VERSION;
        prop_assert_eq!(result.is_ok(), well_formed);
    }

    #[test]
    fn nonce_ciphertext_envelope_decoding_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
        let result = deserialize_vault_key_envelope(&boxed(&bytes));
        prop_assert_eq!(result.is_ok(), bytes.len() >= NONCE_LEN);
    }

    #[test]
    fn password_envelope_decoding_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = decode_password_envelope(&boxed(&bytes));
    }

    #[test]
    fn password_policy_never_panics_on_arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = PasswordServiceImpl::new().validate_password_policy(&boxed(&bytes));
    }

}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn aes_gcm_round_trips_any_plaintext(
        plaintext in proptest::collection::vec(any::<u8>(), 0..8192),
        key in proptest::array::uniform32(any::<u8>()),
    ) {
        let crypto = CryptoServiceImpl::default();
        let decrypted = runtime().block_on(async {
            let payload = crypto.encrypt(&boxed(&plaintext), &boxed(&key)).await?;
            crypto.decrypt(&payload, &boxed(&key)).await
        });
        let decrypted = decrypted.expect("round trip");
        prop_assert_eq!(decrypted.expose_secret(), &plaintext);
    }

    #[test]
    fn aes_gcm_rejects_any_single_bit_flip(
        plaintext in proptest::collection::vec(any::<u8>(), 1..512),
        bit in any::<prop::sample::Index>(),
    ) {
        let crypto = CryptoServiceImpl::default();
        let key = boxed(&[0x11; 32]);
        let outcome = runtime().block_on(async {
            let payload = crypto.encrypt(&boxed(&plaintext), &key).await.expect("encrypt");
            let mut bytes = payload.nonce.to_vec();
            bytes.extend_from_slice(payload.ciphertext.expose_secret());
            let position = bit.index(bytes.len() * 8);
            bytes[position / 8] ^= 1 << (position % 8);
            let mut nonce = [0_u8; NONCE_LEN];
            nonce.copy_from_slice(&bytes[..NONCE_LEN]);
            let altered = EncryptedPayload {
                nonce,
                ciphertext: boxed(&bytes[NONCE_LEN..]),
            };
            crypto.decrypt(&altered, &key).await
        });
        prop_assert!(outcome.is_err(), "an altered payload must never decrypt");
    }

    #[test]
    fn aes_gcm_rejects_any_other_key(
        plaintext in proptest::collection::vec(any::<u8>(), 0..256),
        key in proptest::array::uniform32(any::<u8>()),
        other in proptest::array::uniform32(any::<u8>()),
    ) {
        prop_assume!(key != other);
        let crypto = CryptoServiceImpl::default();
        let outcome = runtime().block_on(async {
            let payload = crypto.encrypt(&boxed(&plaintext), &boxed(&key)).await.expect("encrypt");
            crypto.decrypt(&payload, &boxed(&other)).await
        });
        prop_assert!(outcome.is_err());
    }
}

proptest! {
    // Each case runs Argon2id (the PIN cache uses production parameters); keep the count low.
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn a_wrong_pin_never_releases_the_key(pin in "\\PC{0,16}") {
        let master_key = boxed(&[0x5A; 32]);
        let mut cache = PinCache::wrap(&master_key, "4821", Uuid::new_v4()).expect("wrap");
        if pin != "4821" {
            prop_assert!(cache.try_unwrap(&pin).is_err());
        }
    }

    #[test]
    fn a_forged_recovery_key_envelope_never_opens(bytes in proptest::collection::vec(any::<u8>(), 0..160)) {
        let phrase = SecretString::new(
            "abandon ability able about above absent absorb abstract absurd abuse access accident".into(),
        );
        let result = runtime().block_on(open_with_recovery_phrase(&fast_crypto(), &phrase, &boxed(&bytes)));
        prop_assert!(result.is_err());
    }

    #[test]
    fn a_forged_password_envelope_never_unlocks(bytes in proptest::collection::vec(any::<u8>(), 5..200)) {
        if let Ok(envelope) = decode_password_envelope(&boxed(&bytes)) {
            let result = runtime().block_on(unlock_password_envelope(
                &fast_crypto(),
                &envelope,
                &SecretString::new("any-password".into()),
            ));
            // A forged legacy envelope carries its own "master key": only the account-key form
            // authenticates the password, so assert that one never opens.
            if envelope.is_account_key() {
                prop_assert!(!matches!(result, Ok(Some(_))));
            }
        }
    }

    #[test]
    fn importing_arbitrary_bytes_as_a_backup_fails_cleanly(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let dir = tempfile::tempdir().expect("temp dir");
        let hvb = dir.path().join("forged.hvb");
        let target = dir.path().join("restored.db");
        std::fs::write(&hvb, &bytes).expect("write forged backup");
        let backup = BackupServiceImpl::new();
        let phrase = backup.generate_recovery_key().expect("phrase").recovery_phrase;

        let result = backup.import_hvb_with_recovery_key(&hvb, &phrase, &target);

        prop_assert!(result.is_err());
        prop_assert!(!target.exists(), "a failed import must not leave a database behind");
    }
}

#[tokio::test]
async fn a_large_payload_round_trips_and_truncation_is_detected() {
    let crypto = CryptoServiceImpl::default();
    let key = boxed(&[0x42; 32]);
    let plaintext: Vec<u8> = (0..16 * 1024 * 1024_usize)
        .map(|index| (index % 251) as u8)
        .collect();

    let payload = crypto
        .encrypt(&boxed(&plaintext), &key)
        .await
        .expect("encrypt 16 MiB");
    let decrypted = crypto.decrypt(&payload, &key).await.expect("decrypt");
    assert!(decrypted.expose_secret() == &plaintext);

    let ciphertext = payload.ciphertext.expose_secret();
    let truncated = EncryptedPayload {
        nonce: payload.nonce,
        ciphertext: boxed(&ciphertext[..ciphertext.len() - 1]),
    };
    assert!(crypto.decrypt(&truncated, &key).await.is_err());
}

#[tokio::test]
async fn a_backup_truncated_at_any_point_is_rejected() {
    let account = support::bootstrap_account().await;
    account.pool.close().await;
    let backup = BackupServiceImpl::new();
    let hvb = account.dir.path().join("export.hvb");
    backup
        .export_hvb_with_recovery_key(&account.db_path, &hvb, &account.recovery_phrase)
        .expect("export");
    let full = std::fs::read(&hvb).expect("read export");

    for length in [0, 1, 4, 8, 16, 64, full.len() / 2, full.len() - 1] {
        let truncated = account.dir.path().join(format!("truncated-{length}.hvb"));
        let target = account.dir.path().join(format!("restored-{length}.db"));
        std::fs::write(&truncated, &full[..length]).expect("write truncated");

        let result =
            backup.import_hvb_with_recovery_key(&truncated, &account.recovery_phrase, &target);

        assert!(
            result.is_err(),
            "a backup cut at {length} bytes must be rejected"
        );
        assert!(!target.exists());
    }
}

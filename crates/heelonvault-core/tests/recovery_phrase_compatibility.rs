use bip39::{Language, Mnemonic};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use secrecy::{ExposeSecret, SecretString};

const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
const SQLITE_BYTES: &[u8] = b"SQLite format 3\0bip39 compatibility fixture";
// Public test data exported with BIP39 2 before the dependency migration.
const HVB_FIXTURE: &str = concat!(
    "4856424b0200a80000007b226b6466223a226172676f6e326964222c226d5f636f73745f",
    "6b6962223a36353533362c22745f636f7374223a332c22705f636f7374223a312c227361",
    "6c745f623634223a224f78457775303742504b6e597333762b36586b3479695833667765",
    "4b49585a556c68414c5534456a3753513d222c226e6f6e63655f623634223a2230382b",
    "6269574e524f6c70446143716c222c22706c61696e746578745f73697a65223a34337d",
    "e7598111a6ab98bcdbd6ad30f27745e2b0c4b2651157d3ab026e8bca48b6451721da",
    "309547a58f0693123ec930073cafd48bb4fc1e0bcbcc716208",
);
const VERIFIER_FIXTURE: &str = concat!(
    "0176556e7b481b2ffd3f333af6639bb0dc00646a24dab2dc7b2bd1076658c7c37d",
    "8650c4f50204701fb85af8dd88acc6bb04abb35f0d93881e732dac2a571f2900",
);

#[test]
fn english_entropy_vectors_remain_compatible() -> Result<(), Box<dyn std::error::Error>> {
    for (entropy, phrase) in [
        (
            vec![0_u8; 16],
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        ),
        (vec![0_u8; 32], PHRASE),
        (
            vec![255_u8; 32],
            "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote",
        ),
    ] {
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)?;
        assert_eq!(mnemonic.to_string(), phrase);
        let parsed = Mnemonic::parse_in_normalized(Language::English, phrase)?;
        assert_eq!(parsed.to_entropy(), entropy);
    }
    Ok(())
}

#[test]
fn english_parser_rejects_invalid_recovery_phrases() {
    let bad_checksum = ["abandon"; 24].join(" ");
    let unknown_word = PHRASE.replace("art", "notaword");
    for phrase in [
        "",
        "abandon abandon",
        bad_checksum.as_str(),
        unknown_word.as_str(),
    ] {
        assert!(Mnemonic::parse_in_normalized(Language::English, phrase).is_err());
    }
}

#[test]
fn generated_recovery_phrase_keeps_256_bits_of_entropy() -> Result<(), Box<dyn std::error::Error>> {
    let service = BackupServiceImpl::new();
    let bundle = service.generate_recovery_key()?;
    let mnemonic =
        Mnemonic::parse_in_normalized(Language::English, bundle.recovery_phrase.expose_secret())?;
    assert_eq!(mnemonic.word_count(), 24);
    assert_eq!(mnemonic.to_entropy().len(), 32);
    Ok(())
}

#[test]
fn pre_migration_backup_and_verifier_remain_readable() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let backup = dir.path().join("fixture.hvb");
    let restored = dir.path().join("restored.db");
    std::fs::write(&backup, hex::decode(HVB_FIXTURE)?)?;
    let phrase = SecretString::new(PHRASE.into());
    let service = BackupServiceImpl::new();
    let metadata = service.import_hvb_with_recovery_key(&backup, &phrase, &restored)?;
    assert_eq!(std::fs::read(restored)?, SQLITE_BYTES);
    assert_eq!(metadata.plaintext_size, SQLITE_BYTES.len());

    let verifier = hex::decode(VERIFIER_FIXTURE)?;
    assert!(service.verify_recovery_phrase(&phrase, &verifier)?);
    let other = SecretString::new(
        "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote"
            .into(),
    );
    assert!(!service.verify_recovery_phrase(&other, &verifier)?);
    assert!(
        service
            .import_hvb_with_recovery_key(&backup, &other, &dir.path().join("wrong.db"))
            .is_err()
    );
    assert!(!dir.path().join("wrong.db").exists());
    Ok(())
}

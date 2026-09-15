#![allow(clippy::disallowed_methods)]

//! CWE-532: no secret may reach the logs. Every flow that handles a password, a recovery
//! phrase, an account key or a secret value runs here under a TRACE-level subscriber (the most
//! verbose setting a user can enable with `RUST_LOG=trace`), including its failure paths and the
//! `AppError` values the application logs with `error = %error`. The captured output is then
//! searched for every secret, in clear and in its usual encodings.

mod support;

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

use base64::Engine;
use heelonvault_core::repositories::secret_repository::SqlxSecretRepository;
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::SqlxVaultRepository;
use heelonvault_core::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::CryptoServiceImpl;
use heelonvault_core::services::import_service::ImportServiceImpl;
use heelonvault_core::services::recovery_service::reset_master_password_from_backup;
use heelonvault_core::services::totp_service::{SqliteTotpService, TotpService};
use heelonvault_core::services::user_service::{UserService, UserServiceImpl};
use secrecy::{ExposeSecret, SecretString};
use support::{
    PASSWORD, USERNAME, bootstrap_account, create_vault, fast_crypto, insert_secret, secret,
};
use tracing_subscriber::fmt::MakeWriter;

const NEW_PASSWORD: &str = "Brand-New-Master-Password-2026!";
const WRONG_PASSWORD: &str = "Wrong-Password-Typed-By-Mistake-99";
const SECRET_VALUE: &str = "vault-entry-plaintext-value-7f3a";
const CSV_PASSWORD: &str = "Csv-Imported-P@ssw0rd-In-A-Bad-Row";

// ─── capture ────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("capture poisoned"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = Capture;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// One global TRACE subscriber for the whole binary: tests run in parallel and a secret from
/// any of them must be absent from everything captured.
fn capture() -> &'static Capture {
    static CAPTURE: OnceLock<Capture> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let capture = Capture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(capture.clone())
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("global subscriber");
        capture
    })
}

fn log_error(context: &str, error: &dyn std::fmt::Debug) {
    tracing::error!(context, error = ?error, "flow failed");
}

fn assert_logs_never_contain(label: &str, secret: &[u8]) {
    let logs = capture().0.lock().expect("capture").clone();
    let encodings = [
        secret.to_vec(),
        hex::encode(secret).into_bytes(),
        hex::encode_upper(secret).into_bytes(),
        base64::engine::general_purpose::STANDARD
            .encode(secret)
            .into_bytes(),
        format!("{secret:?}").into_bytes(),
    ];
    for encoded in encodings {
        assert!(
            !support::contains(&logs, &encoded),
            "{label} leaked into the logs ({} bytes captured)",
            logs.len()
        );
    }
}

// ─── flows ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn authentication_and_password_change_never_log_a_secret() {
    capture();
    tracing::info!("capture-marker-authentication");
    let account = bootstrap_account().await;
    let auth = Arc::new(AuthServiceImpl::new(fast_crypto()));
    let envelope = SqlxUserRepository::new(account.pool.clone())
        .get_password_envelope_by_user_id(account.user_id)
        .await
        .expect("query")
        .expect("envelope");
    auth.upsert_password_envelope(USERNAME, envelope)
        .await
        .expect("load credentials");

    let wrong = auth
        .derive_key_if_valid(USERNAME, secret(WRONG_PASSWORD))
        .await;
    assert!(matches!(wrong, Ok(None)));

    let service = UserServiceImpl::new(
        SqlxUserRepository::new(account.pool.clone()),
        SqlxVaultRepository::new(account.pool.clone()),
        SqlxVaultRepository::new(account.pool.clone()),
        SqlxSecretRepository::new(account.pool.clone()),
        Arc::clone(&auth),
        fast_crypto(),
    );
    if let Err(error) = service
        .change_master_password(
            account.user_id,
            secret(WRONG_PASSWORD),
            secret(NEW_PASSWORD),
        )
        .await
    {
        log_error("change password with a wrong current password", &error);
        tracing::error!(error = %error, "displayed error");
    } else {
        panic!("a wrong current password must be refused");
    }
    if let Err(error) = service
        .change_master_password(account.user_id, secret(PASSWORD), secret("short"))
        .await
    {
        log_error("change password to a weak one", &error);
    }
    let new_key = service
        .change_master_password(account.user_id, secret(PASSWORD), secret(NEW_PASSWORD))
        .await
        .expect("password change");

    assert_logs_never_contain("the password", PASSWORD.as_bytes());
    assert_logs_never_contain("a mistyped password", WRONG_PASSWORD.as_bytes());
    assert_logs_never_contain("the new password", NEW_PASSWORD.as_bytes());
    assert_logs_never_contain("the account key", &account.account_key);
    assert_logs_never_contain("the session key", new_key.expose_secret());
    assert_logs_never_contain(
        "the recovery phrase",
        account.recovery_phrase.expose_secret().as_bytes(),
    );
    let logs = capture().0.lock().expect("capture").clone();
    assert!(
        support::contains(&logs, b"capture-marker-authentication"),
        "the subscriber must actually capture events, or this test proves nothing"
    );
}

#[tokio::test]
async fn backup_and_recovery_failures_never_log_a_secret() {
    capture();
    let account = bootstrap_account().await;
    let vault_key = [3_u8; 32];
    let vault_id = create_vault(
        &account.pool,
        account.user_id,
        &account.account_key,
        &vault_key,
    )
    .await;
    let (_, ciphertext) =
        insert_secret(&account.pool, vault_id, &vault_key, SECRET_VALUE.as_bytes()).await;
    account.pool.close().await;

    let backup = BackupServiceImpl::new();
    let hvb = account.dir.path().join("export.hvb");
    backup
        .export_hvb_with_recovery_key(&account.db_path, &hvb, &account.recovery_phrase)
        .expect("export");
    let other_phrase = backup
        .generate_recovery_key()
        .expect("phrase")
        .recovery_phrase;

    let wrong_phrase = backup.import_hvb_with_recovery_key(
        &hvb,
        &other_phrase,
        &account.dir.path().join("wrong.db"),
    );
    log_error("import with another phrase", &wrong_phrase);
    let invalid_phrase = backup.import_hvb_with_recovery_key(
        &hvb,
        &SecretString::new("not a bip39 phrase at all".into()),
        &account.dir.path().join("invalid.db"),
    );
    log_error("import with an invalid phrase", &invalid_phrase);

    let mut tampered = std::fs::read(&hvb).expect("read export");
    let middle = tampered.len() / 2;
    tampered[middle] ^= 0x55;
    let tampered_path = account.dir.path().join("tampered.hvb");
    std::fs::write(&tampered_path, tampered).expect("write tampered");
    let altered = backup.import_hvb_with_recovery_key(
        &tampered_path,
        &account.recovery_phrase,
        &account.dir.path().join("tampered.db"),
    );
    log_error("import of an altered file", &altered);

    let pool = support::open_pool(&account.db_path).await;
    let reset = reset_master_password_from_backup(
        &pool,
        &fast_crypto(),
        &other_phrase,
        &SecretString::new(NEW_PASSWORD.into()),
    )
    .await;
    log_error("reset with another phrase", &reset);
    pool.close().await;

    assert_logs_never_contain(
        "the recovery phrase",
        account.recovery_phrase.expose_secret().as_bytes(),
    );
    assert_logs_never_contain(
        "a rejected recovery phrase",
        other_phrase.expose_secret().as_bytes(),
    );
    assert_logs_never_contain("a secret value", SECRET_VALUE.as_bytes());
    assert_logs_never_contain("a stored ciphertext", &ciphertext);
    assert_logs_never_contain("the vault key", &vault_key);
    assert_logs_never_contain("the account key", &account.account_key);
    assert_logs_never_contain("a new password", NEW_PASSWORD.as_bytes());
}

#[tokio::test]
async fn second_factor_verification_never_logs_the_totp_secret() {
    capture();
    let account = bootstrap_account().await;
    let auth = Arc::new(AuthServiceImpl::new(fast_crypto()));
    let envelope = SqlxUserRepository::new(account.pool.clone())
        .get_password_envelope_by_user_id(account.user_id)
        .await
        .expect("query")
        .expect("envelope");
    auth.upsert_password_envelope(USERNAME, envelope)
        .await
        .expect("load credentials");
    let totp: SqliteTotpService<AuthServiceImpl<CryptoServiceImpl>, CryptoServiceImpl> =
        SqliteTotpService::new(account.pool.clone(), auth, fast_crypto(), "HeelonVault");
    let payload = totp.create_setup_payload(USERNAME).expect("setup");

    let refused = totp
        .enable_totp(
            account.user_id,
            USERNAME,
            &secrecy::SecretBox::new(Box::new(account.account_key.clone())),
            &payload.base32_secret,
            "000000",
        )
        .await;
    log_error("enable TOTP with a wrong code", &refused);
    let login = totp
        .verify_login_totp(USERNAME, secret(WRONG_PASSWORD), "123456")
        .await;
    log_error("login TOTP with a wrong password", &login);

    assert_logs_never_contain("the TOTP secret", payload.base32_secret.as_bytes());
    assert_logs_never_contain("the otpauth URL", payload.otpauth_url.as_bytes());
    assert_logs_never_contain("a mistyped password", WRONG_PASSWORD.as_bytes());
}

#[tokio::test]
async fn csv_import_preview_never_logs_imported_passwords() {
    capture();
    let dir = tempfile::tempdir().expect("temp dir");
    let csv = dir.path().join("import.csv");
    std::fs::write(
        &csv,
        format!(
            "name,url,username,password,notes,category,tags\n\
             Ok,https://ok.example.org,bob,{CSV_PASSWORD},,,\n\
             Bad,https://bad.example.org,bob,{CSV_PASSWORD},,,,EXTRA\n\
             Ftp,ftp://nope.example.org,bob,{CSV_PASSWORD},,,\n"
        ),
    )
    .expect("write csv");

    let preview = ImportServiceImpl::preview_csv(&csv);
    log_error("csv preview", &preview);
    let preview = preview.expect("preview");
    assert_eq!(preview.importable_rows, 1);
    assert_eq!(preview.failed_rows, 2);

    assert_logs_never_contain("an imported password", CSV_PASSWORD.as_bytes());
}

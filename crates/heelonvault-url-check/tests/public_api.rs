//! Deterministic checks of the public API. None of these cases reaches the
//! network: every input is rejected before DNS resolution or connection.

use heelonvault_url_check::{CheckError, check_https};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Runs a test body on a fresh current-thread runtime with I/O and timers.
fn run<F: std::future::Future<Output = TestResult>>(body: F) -> TestResult {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(body)
}

#[test]
fn rejects_invalid_urls() -> TestResult {
    run(async {
        for input in [
            "",
            "not a url",
            "example.com",
            "https://",
            "https://example.com:0/",
        ] {
            assert_eq!(
                check_https(input).await,
                Err(CheckError::InvalidUrl),
                "{input:?}"
            );
        }
        Ok(())
    })
}

#[test]
fn requires_https() -> TestResult {
    run(async {
        for input in [
            "http://example.com/",
            "ftp://example.com/",
            "file:///etc/hosts",
        ] {
            assert_eq!(
                check_https(input).await,
                Err(CheckError::HttpsRequired),
                "{input:?}"
            );
        }
        Ok(())
    })
}

#[test]
fn rejects_credentials() -> TestResult {
    run(async {
        for input in [
            "https://user@example.com/",
            "https://user:secret@example.com/",
        ] {
            assert_eq!(
                check_https(input).await,
                Err(CheckError::CredentialsNotAllowed),
                "{input:?}"
            );
        }
        Ok(())
    })
}

#[test]
fn rejects_non_public_destinations_without_network() -> TestResult {
    run(async {
        for input in [
            "https://localhost/",
            "https://router.local/",
            "https://127.0.0.1/",
            "https://10.0.0.1/",
            "https://169.254.169.254/latest/meta-data",
            "https://192.168.1.1:8443/",
            "https://100.64.0.1/",
            "https://192.0.2.10/",
            "https://224.0.0.1/",
            "https://0.0.0.0/",
            "https://[::1]/",
            "https://[::]/",
            "https://[::ffff:127.0.0.1]/",
            "https://[::ffff:a00:1]/",
            "https://[fe80::1]/",
            "https://[fd00::1]/",
            "https://[2001:db8::1]/",
            "https://[ff02::1]/",
            "https://2130706433/",
            "https://0x7f.1/",
        ] {
            assert_eq!(
                check_https(input).await,
                Err(CheckError::NonPublicAddress),
                "{input:?}"
            );
        }
        Ok(())
    })
}

#[test]
fn error_display_is_generic() {
    let all = [
        CheckError::InvalidUrl,
        CheckError::HttpsRequired,
        CheckError::CredentialsNotAllowed,
        CheckError::NonPublicAddress,
        CheckError::Resolution,
        CheckError::Connection,
        CheckError::Tls,
        CheckError::Timeout,
    ];
    for error in all {
        let text = error.to_string();
        assert!(!text.is_empty());
        assert!(!text.contains("://"));
    }
}

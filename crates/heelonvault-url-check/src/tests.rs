use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::{RootCertStore, ServerConfig};

use super::*;
use crate::ip_filter::is_public_ip;
use crate::target::MAX_URL_LEN;

type TestResult = Result<(), Box<dyn Error>>;

/// Runs a test body on a fresh current-thread runtime with I/O and timers.
fn run<F: std::future::Future<Output = TestResult>>(body: F) -> TestResult {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(body)
}

const CA_PEM: &[u8] = include_bytes!("../tests/fixtures/ca.pem.fixture");
const LEAF_PEM: &[u8] = include_bytes!("../tests/fixtures/leaf.pem.fixture");
const LEAF_KEY: &[u8] = include_bytes!("../tests/fixtures/leaf.key.fixture");
const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

fn ip(text: &str) -> Result<IpAddr, Box<dyn Error>> {
    Ok(text.parse()?)
}

// ---------------------------------------------------------------- IP filter

#[test]
fn rejects_non_public_ipv4() -> TestResult {
    for text in [
        "0.0.0.0",
        "0.1.2.3",
        "10.0.0.1",
        "10.255.255.255",
        "100.64.0.1",
        "100.127.255.254",
        "127.0.0.1",
        "127.255.255.255",
        "169.254.169.254",
        "172.16.0.1",
        "172.31.255.255",
        "192.0.0.8",
        "192.0.2.1",
        "192.31.196.1",
        "192.52.193.1",
        "192.88.99.1",
        "192.168.1.1",
        "192.175.48.1",
        "198.18.0.1",
        "198.19.255.255",
        "198.51.100.7",
        "203.0.113.9",
        "224.0.0.1",
        "239.255.255.250",
        "240.0.0.1",
        "255.255.255.255",
    ] {
        assert!(!is_public_ip(ip(text)?), "{text} must be rejected");
    }
    Ok(())
}

#[test]
fn accepts_public_ipv4() -> TestResult {
    for text in [
        "1.1.1.1",
        "8.8.8.8",
        "9.255.255.255",
        "11.0.0.1",
        "100.63.255.255",
        "100.128.0.1",
        "172.15.255.255",
        "172.32.0.1",
        "192.0.1.1",
        "192.0.3.1",
        "192.169.0.1",
        "198.17.255.255",
        "198.20.0.1",
        "223.255.255.254",
    ] {
        assert!(is_public_ip(ip(text)?), "{text} must be accepted");
    }
    Ok(())
}

#[test]
fn rejects_non_public_ipv6() -> TestResult {
    for text in [
        "::",
        "::1",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "::ffff:169.254.169.254",
        "::ffff:192.168.0.1",
        "::ffff:0.0.0.0",
        "::127.0.0.1",
        "::8.8.8.8",
        "64:ff9b::127.0.0.1",
        "64:ff9b::a00:1",
        "64:ff9b:1::1",
        "100::1",
        "2001::1",
        "2001:2::1",
        "2001:10::1",
        "2001:db8::1",
        "2001:db8:ffff::1",
        "2002:c0a8:101::1",
        "3fff::1",
        "3fff:fff::1",
        "fc00::1",
        "fd12:3456::1",
        "fe80::1",
        "febf::1",
        "fec0::1",
        "ff02::1",
        "ff0e::1",
        "4000::1",
        "5f00::1",
    ] {
        assert!(!is_public_ip(ip(text)?), "{text} must be rejected");
    }
    Ok(())
}

#[test]
fn accepts_public_ipv6() -> TestResult {
    for text in [
        "2606:4700:4700::1111",
        "2001:4860:4860::8888",
        "2a00:1450::1",
        "2001:200::1",
        "2003::1",
        "3ffe::1",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
    ] {
        assert!(is_public_ip(ip(text)?), "{text} must be accepted");
    }
    Ok(())
}

#[test]
fn std_special_addresses_are_rejected() {
    for candidate in [
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::BROADCAST),
        IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped()),
        IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_compatible()),
    ] {
        assert!(!is_public_ip(candidate), "{candidate} must be rejected");
    }
}

// ---------------------------------------------------------------- URL parsing

#[test]
fn url_keeps_only_host_and_port() -> TestResult {
    let parsed = target::parse_target("https://Example.COM./a/b?token=x#frag")?;
    assert_eq!(
        parsed,
        Target {
            host: TargetHost::Domain("example.com".to_owned()),
            port: 443
        }
    );
    let parsed = target::parse_target("https://exämple.com:8443/")?;
    assert_eq!(
        parsed,
        Target {
            host: TargetHost::Domain("xn--exmple-cua.com".to_owned()),
            port: 8443
        }
    );
    let parsed = target::parse_target("https://[2606:4700:4700::1111]/")?;
    assert_eq!(parsed.host, TargetHost::Ip(ip("2606:4700:4700::1111")?));
    Ok(())
}

#[test]
fn url_validation_errors() {
    let cases: &[(&str, CheckError)] = &[
        ("", CheckError::InvalidUrl),
        ("not a url", CheckError::InvalidUrl),
        ("example.com", CheckError::InvalidUrl),
        ("https://", CheckError::InvalidUrl),
        ("https://exa mple.com/", CheckError::InvalidUrl),
        ("https://example.com:0/", CheckError::InvalidUrl),
        ("https://example.com:99999/", CheckError::InvalidUrl),
        ("https://./", CheckError::InvalidUrl),
        ("http://example.com/", CheckError::HttpsRequired),
        ("ftp://example.com/", CheckError::HttpsRequired),
        ("wss://example.com/", CheckError::HttpsRequired),
        ("file:///etc/passwd", CheckError::HttpsRequired),
        ("mailto:a@example.com", CheckError::HttpsRequired),
        (
            "https://user@example.com/",
            CheckError::CredentialsNotAllowed,
        ),
        (
            "https://user:pw@example.com/",
            CheckError::CredentialsNotAllowed,
        ),
        (
            "https://:pw@example.com/",
            CheckError::CredentialsNotAllowed,
        ),
        ("https://localhost/", CheckError::NonPublicAddress),
        ("https://LOCALHOST./", CheckError::NonPublicAddress),
        ("https://api.localhost/", CheckError::NonPublicAddress),
        ("https://printer.local/", CheckError::NonPublicAddress),
        ("https://nas.home.arpa/", CheckError::NonPublicAddress),
        ("https://svc.internal/", CheckError::NonPublicAddress),
    ];
    for (input, expected) in cases {
        assert_eq!(
            target::parse_target(input).err(),
            Some(*expected),
            "input {input:?}"
        );
    }
}

#[test]
fn local_name_suffix_requires_label_boundary() -> TestResult {
    let parsed = target::parse_target("https://notlocalhost.com/")?;
    assert_eq!(
        parsed.host,
        TargetHost::Domain("notlocalhost.com".to_owned())
    );
    let parsed = target::parse_target("https://mylocal/")?;
    assert_eq!(parsed.host, TargetHost::Domain("mylocal".to_owned()));
    Ok(())
}

#[test]
fn overlong_url_is_rejected() {
    let long = format!("https://example.com/{}", "a".repeat(MAX_URL_LEN));
    assert_eq!(target::parse_target(&long), Err(CheckError::InvalidUrl));
}

#[test]
fn error_messages_never_contain_input() {
    let secret = "https://user:hunter2@example.com/private?token=abc";
    let error = target::parse_target(secret).err();
    assert_eq!(error, Some(CheckError::CredentialsNotAllowed));
    let rendered = format!("{error:?} {}", CheckError::CredentialsNotAllowed);
    assert!(!rendered.contains("hunter2"));
    assert!(!rendered.contains("example.com"));
    assert!(!rendered.contains("token"));
}

// ---------------------------------------------------------------- DNS validation

#[test]
fn every_resolved_address_must_be_public() -> TestResult {
    let mixed = vec![ip("8.8.8.8")?, ip("10.0.0.1")?];
    assert_eq!(
        validate_resolved(mixed, AddressPolicy::PublicOnly),
        Err(CheckError::NonPublicAddress)
    );
    let mapped = vec![ip("2606:4700:4700::1111")?, ip("::ffff:192.168.0.1")?];
    assert_eq!(
        validate_resolved(mapped, AddressPolicy::PublicOnly),
        Err(CheckError::NonPublicAddress)
    );
    assert_eq!(
        validate_resolved(Vec::new(), AddressPolicy::PublicOnly),
        Err(CheckError::Resolution)
    );
    let duplicated = vec![ip("8.8.8.8")?, ip("8.8.8.8")?, ip("1.1.1.1")?];
    assert_eq!(
        validate_resolved(duplicated, AddressPolicy::PublicOnly),
        Ok(vec![ip("8.8.8.8")?, ip("1.1.1.1")?])
    );
    Ok(())
}

// ---------------------------------------------------------------- Local fixtures

fn fixture_roots() -> Result<Arc<RootCertStore>, Box<dyn Error>> {
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from_pem_slice(CA_PEM)?)?;
    Ok(Arc::new(roots))
}

fn options(resolver: Resolver, policy: AddressPolicy) -> Result<Options, Box<dyn Error>> {
    Ok(Options {
        resolver,
        policy,
        roots: fixture_roots()?,
        total_timeout: Duration::from_secs(5),
        attempt_timeout: Duration::from_secs(3),
    })
}

/// What the fixture server observed from the client.
#[derive(Debug)]
struct Observed {
    sni: Option<String>,
    alpn_offered: bool,
    application_bytes: Vec<u8>,
}

async fn tls_fixture() -> Result<(u16, JoinHandle<Option<Observed>>), Box<dyn Error>> {
    let certs = vec![CertificateDer::from_pem_slice(LEAF_PEM)?];
    let key = PrivateKeyDer::from_pem_slice(LEAF_KEY)?;
    let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind((LOOPBACK, 0)).await?;
    let port = listener.local_addr()?.port();
    let handle = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.ok()?;
        let mut tls = acceptor.accept(tcp).await.ok()?;
        let (_, connection) = tls.get_ref();
        let sni = connection.server_name().map(str::to_owned);
        let alpn_offered = connection.alpn_protocol().is_some();
        let mut application_bytes = Vec::new();
        let _ = timeout(
            Duration::from_secs(2),
            tls.read_to_end(&mut application_bytes),
        )
        .await;
        Some(Observed {
            sni,
            alpn_offered,
            application_bytes,
        })
    });
    Ok((port, handle))
}

#[test]
fn handshake_succeeds_without_sending_request_data() -> TestResult {
    run(async {
        let (port, server) = tls_fixture().await?;
        let url = format!("https://fixture.test:{port}/secret/path?token=abc#frag");
        let opts = options(
            Resolver::Static(vec![LOOPBACK]),
            AddressPolicy::PublicOrIpv4LoopbackForTests,
        )?;
        assert_eq!(run_check(&url, &opts).await, Ok(()));

        let observed = server
            .await?
            .ok_or("fixture did not complete a handshake")?;
        assert_eq!(observed.sni.as_deref(), Some("fixture.test"));
        assert!(!observed.alpn_offered);
        assert!(
            observed.application_bytes.is_empty(),
            "client must not send application data"
        );
        Ok(())
    })
}

#[test]
fn hostname_mismatch_fails_tls() -> TestResult {
    run(async {
        let (port, _server) = tls_fixture().await?;
        let url = format!("https://other.test:{port}/");
        let opts = options(
            Resolver::Static(vec![LOOPBACK]),
            AddressPolicy::PublicOrIpv4LoopbackForTests,
        )?;
        assert_eq!(run_check(&url, &opts).await, Err(CheckError::Tls));
        Ok(())
    })
}

#[test]
fn ip_literal_without_ip_san_fails_tls() -> TestResult {
    run(async {
        let (port, _server) = tls_fixture().await?;
        let url = format!("https://127.0.0.1:{port}/");
        let opts = options(
            Resolver::System,
            AddressPolicy::PublicOrIpv4LoopbackForTests,
        )?;
        assert_eq!(run_check(&url, &opts).await, Err(CheckError::Tls));
        Ok(())
    })
}

#[test]
fn untrusted_root_fails_tls() -> TestResult {
    run(async {
        let (port, _server) = tls_fixture().await?;
        let url = format!("https://fixture.test:{port}/");
        let mut opts = options(
            Resolver::Static(vec![LOOPBACK]),
            AddressPolicy::PublicOrIpv4LoopbackForTests,
        )?;
        opts.roots = Options::production().roots;
        assert_eq!(run_check(&url, &opts).await, Err(CheckError::Tls));
        Ok(())
    })
}

#[test]
fn production_policy_never_connects_to_loopback() -> TestResult {
    run(async {
        let listener = TcpListener::bind((LOOPBACK, 0)).await?;
        let port = listener.local_addr()?.port();
        let strict = options(Resolver::Static(vec![LOOPBACK]), AddressPolicy::PublicOnly)?;
        assert_eq!(
            run_check(&format!("https://fixture.test:{port}/"), &strict).await,
            Err(CheckError::NonPublicAddress)
        );
        assert_eq!(
            run_check(
                &format!("https://127.0.0.1:{port}/"),
                &Options::production()
            )
            .await,
            Err(CheckError::NonPublicAddress)
        );
        assert_eq!(
            run_check(
                &format!("https://[::ffff:127.0.0.1]:{port}/"),
                &Options::production()
            )
            .await,
            Err(CheckError::NonPublicAddress)
        );
        // Rebinding-style answer: one public and one loopback record fails closed.
        let mixed = options(
            Resolver::Static(vec![ip("8.8.8.8")?, LOOPBACK]),
            AddressPolicy::PublicOnly,
        )?;
        assert_eq!(
            run_check(&format!("https://fixture.test:{port}/"), &mixed).await,
            Err(CheckError::NonPublicAddress)
        );
        let accepted = timeout(Duration::from_millis(200), listener.accept()).await;
        assert!(accepted.is_err(), "no connection may reach the listener");
        Ok(())
    })
}

#[test]
fn oversized_dns_answer_fails_closed() -> TestResult {
    run(async {
        let answers = (0..=MAX_DNS_RESULTS)
            .map(|i| IpAddr::V4(Ipv4Addr::new(8, 8, 8, u8::try_from(i).unwrap_or(u8::MAX))))
            .collect();
        let opts = options(Resolver::Static(answers), AddressPolicy::PublicOnly)?;
        assert_eq!(
            run_check("https://fixture.test/", &opts).await,
            Err(CheckError::Resolution)
        );
        let opts = options(Resolver::Static(Vec::new()), AddressPolicy::PublicOnly)?;
        assert_eq!(
            run_check("https://fixture.test/", &opts).await,
            Err(CheckError::Resolution)
        );
        Ok(())
    })
}

#[test]
fn stalled_resolver_hits_total_timeout() -> TestResult {
    run(async {
        let mut opts = options(Resolver::Hang, AddressPolicy::PublicOnly)?;
        opts.total_timeout = Duration::from_millis(100);
        assert_eq!(
            run_check("https://fixture.test/", &opts).await,
            Err(CheckError::Timeout)
        );
        Ok(())
    })
}

#[test]
fn silent_server_times_out() -> TestResult {
    run(async {
        let listener = TcpListener::bind((LOOPBACK, 0)).await?;
        let port = listener.local_addr()?.port();
        let _holder = tokio::spawn(async move {
            let accepted = listener.accept().await;
            tokio::time::sleep(Duration::from_secs(5)).await;
            drop(accepted);
        });
        let mut opts = options(
            Resolver::Static(vec![LOOPBACK]),
            AddressPolicy::PublicOrIpv4LoopbackForTests,
        )?;
        opts.attempt_timeout = Duration::from_millis(200);
        assert_eq!(
            run_check(&format!("https://fixture.test:{port}/"), &opts).await,
            Err(CheckError::Timeout)
        );
        Ok(())
    })
}

#[test]
fn non_tls_server_fails_tls() -> TestResult {
    run(async {
        let listener = TcpListener::bind((LOOPBACK, 0)).await?;
        let port = listener.local_addr()?.port();
        let _server = tokio::spawn(async move {
            if let Ok((tcp, _)) = listener.accept().await {
                drop(tcp);
            }
        });
        let opts = options(
            Resolver::Static(vec![LOOPBACK]),
            AddressPolicy::PublicOrIpv4LoopbackForTests,
        )?;
        assert_eq!(
            run_check(&format!("https://fixture.test:{port}/"), &opts).await,
            Err(CheckError::Tls)
        );
        Ok(())
    })
}

#[test]
fn closed_port_fails_connection() -> TestResult {
    run(async {
        let port = {
            let listener = TcpListener::bind((LOOPBACK, 0)).await?;
            listener.local_addr()?.port()
        };
        let opts = options(
            Resolver::Static(vec![LOOPBACK]),
            AddressPolicy::PublicOrIpv4LoopbackForTests,
        )?;
        assert_eq!(
            run_check(&format!("https://fixture.test:{port}/"), &opts).await,
            Err(CheckError::Connection)
        );
        Ok(())
    })
}

#[test]
fn dropping_a_pending_check_abandons_it() -> TestResult {
    // Uses the synthetic `Resolver::Hang`; this does not claim that a system
    // DNS lookup already in flight is interrupted (it is not).
    run(async {
        let opts = options(Resolver::Hang, AddressPolicy::PublicOnly)?;
        let check = run_check("https://fixture.test/", &opts);
        let outcome = timeout(Duration::from_millis(50), check).await;
        assert!(
            outcome.is_err(),
            "check must still be pending when the timeout drops it"
        );
        Ok(())
    })
}

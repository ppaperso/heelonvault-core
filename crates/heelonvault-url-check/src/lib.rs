//! Optional, bounded HTTPS reachability check for HeelonVault.
//!
//! [`check_https`] verifies that the host of an `https://` URL resolves to
//! public addresses only and completes a TLS handshake with a certificate valid
//! for that host. It never speaks HTTP: no path, query, fragment, user info,
//! cookie or header is ever sent. The only data leaving the machine are the DNS
//! query for the host name and a TLS `ClientHello` (with SNI for domain names).
//!
//! The check is fully isolated from `heelonvault-core`, which stays network-free.
//! Callers are responsible for obtaining user consent before invoking it.

#![forbid(unsafe_code)]

mod ip_filter;
mod target;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::{Instant, timeout, timeout_at};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::client::Resumption;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

use crate::target::{Target, TargetHost, parse_target};

/// Total time budget for one check (DNS resolution + TCP connect + TLS handshake).
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(8);
/// Time budget for a single TCP connect + TLS handshake attempt.
pub const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(4);
/// Maximum number of DNS results accepted; larger answers fail closed.
pub const MAX_DNS_RESULTS: usize = 16;
/// Maximum number of resolved addresses a connection is attempted to.
pub const MAX_CONNECT_ATTEMPTS: usize = 2;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);

/// Outcome of a failed check.
///
/// Variants carry no detail on purpose: neither the URL nor resolved addresses
/// nor low-level error messages are exposed, so the error can be logged or
/// displayed without leaking user data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum CheckError {
    /// The input is not a valid absolute URL with a host.
    #[error("invalid URL")]
    InvalidUrl,
    /// The URL scheme is not `https`.
    #[error("only https URLs can be checked")]
    HttpsRequired,
    /// The URL contains a user name or password.
    #[error("URLs containing credentials are not checked")]
    CredentialsNotAllowed,
    /// The host is, or resolves to at least one, non-public address.
    #[error("destination is not a public address")]
    NonPublicAddress,
    /// The host name could not be resolved, or the answer was unusable.
    #[error("host name resolution failed")]
    Resolution,
    /// No TCP connection could be established.
    #[error("connection failed")]
    Connection,
    /// The TLS handshake or certificate validation failed.
    #[error("TLS handshake failed")]
    Tls,
    /// The overall time budget was exhausted.
    #[error("check timed out")]
    Timeout,
}

/// Checks that `raw_url` designates a public HTTPS endpoint presenting a
/// certificate valid for its host.
///
/// Must be called from within a Tokio runtime (with the time and I/O drivers
/// enabled). The whole check is bounded by [`TOTAL_TIMEOUT`]. Dropping the
/// returned future abandons the check: no connection or TLS handshake is
/// started afterwards and any in-flight socket is closed. A system DNS lookup
/// already in flight runs on Tokio's blocking pool and cannot be interrupted;
/// it may still complete in the background, but its result is discarded.
pub async fn check_https(raw_url: &str) -> Result<(), CheckError> {
    run_check(raw_url, &Options::production()).await
}

#[derive(Clone)]
enum Resolver {
    System,
    #[cfg(test)]
    Static(Vec<IpAddr>),
    /// Test-only resolver that never completes. It stands in for a stalled
    /// lookup in timeout/drop tests. It does not model the system resolver,
    /// whose blocking lookup is not interrupted on drop.
    #[cfg(test)]
    Hang,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AddressPolicy {
    PublicOnly,
    /// Test-only: additionally admits `127.0.0.1` so local TLS fixtures can be
    /// reached. Never compiled into non-test builds.
    #[cfg(test)]
    PublicOrIpv4LoopbackForTests,
}

impl AddressPolicy {
    fn allows(self, ip: IpAddr) -> bool {
        match self {
            Self::PublicOnly => ip_filter::is_public_ip(ip),
            #[cfg(test)]
            Self::PublicOrIpv4LoopbackForTests => {
                ip_filter::is_public_ip(ip) || ip == IpAddr::from([127, 0, 0, 1])
            }
        }
    }
}

struct Options {
    resolver: Resolver,
    policy: AddressPolicy,
    roots: Arc<RootCertStore>,
    total_timeout: Duration,
    attempt_timeout: Duration,
}

impl Options {
    fn production() -> Self {
        Self {
            resolver: Resolver::System,
            policy: AddressPolicy::PublicOnly,
            roots: Arc::new(RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            }),
            total_timeout: TOTAL_TIMEOUT,
            attempt_timeout: ATTEMPT_TIMEOUT,
        }
    }
}

async fn run_check(raw_url: &str, options: &Options) -> Result<(), CheckError> {
    let target = parse_target(raw_url)?;
    let deadline = Instant::now() + options.total_timeout;
    let stream = match timeout_at(deadline, connect_and_handshake(&target, options)).await {
        Ok(result) => result?,
        Err(_) => return Err(CheckError::Timeout),
    };
    close(stream).await;
    Ok(())
}

async fn connect_and_handshake(
    target: &Target,
    options: &Options,
) -> Result<TlsStream<TcpStream>, CheckError> {
    let (server_name, addresses) = match &target.host {
        TargetHost::Ip(ip) => {
            if !options.policy.allows(*ip) {
                return Err(CheckError::NonPublicAddress);
            }
            (ServerName::IpAddress((*ip).into()), vec![*ip])
        }
        TargetHost::Domain(domain) => {
            let server_name =
                ServerName::try_from(domain.clone()).map_err(|_| CheckError::InvalidUrl)?;
            let addresses = resolve(domain, target.port, options).await?;
            (server_name, addresses)
        }
    };

    let connector = tls_connector(options)?;
    let mut last_error = CheckError::Connection;
    for ip in addresses.into_iter().take(MAX_CONNECT_ATTEMPTS) {
        // The socket address is pinned: the TCP connection goes to the exact,
        // already validated IP and the host name is never resolved again.
        let socket = SocketAddr::new(ip, target.port);
        let attempt = attempt(&connector, socket, server_name.clone());
        match timeout(options.attempt_timeout, attempt).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last_error = error,
            Err(_) => last_error = CheckError::Timeout,
        }
    }
    Err(last_error)
}

async fn attempt(
    connector: &TlsConnector,
    socket: SocketAddr,
    server_name: ServerName<'static>,
) -> Result<TlsStream<TcpStream>, CheckError> {
    let tcp = TcpStream::connect(socket)
        .await
        .map_err(|_| CheckError::Connection)?;
    // Disabling Nagle is best effort; failing to do so does not affect the check.
    let _ = tcp.set_nodelay(true);
    connector
        .connect(server_name, tcp)
        .await
        .map_err(|_| CheckError::Tls)
}

/// Resolves `domain` and validates every returned address (fail closed).
async fn resolve(domain: &str, port: u16, options: &Options) -> Result<Vec<IpAddr>, CheckError> {
    let raw: Vec<IpAddr> = match &options.resolver {
        Resolver::System => {
            let answers = tokio::net::lookup_host((domain, port))
                .await
                .map_err(|_| CheckError::Resolution)?;
            let mut collected = Vec::new();
            for answer in answers {
                if collected.len() == MAX_DNS_RESULTS {
                    return Err(CheckError::Resolution);
                }
                collected.push(answer.ip());
            }
            collected
        }
        #[cfg(test)]
        Resolver::Static(ips) => {
            if ips.len() > MAX_DNS_RESULTS {
                return Err(CheckError::Resolution);
            }
            ips.clone()
        }
        #[cfg(test)]
        Resolver::Hang => std::future::pending().await,
    };
    validate_resolved(raw, options.policy)
}

fn validate_resolved(raw: Vec<IpAddr>, policy: AddressPolicy) -> Result<Vec<IpAddr>, CheckError> {
    if raw.is_empty() {
        return Err(CheckError::Resolution);
    }
    if raw.iter().any(|ip| !policy.allows(*ip)) {
        return Err(CheckError::NonPublicAddress);
    }
    let mut unique: Vec<IpAddr> = Vec::with_capacity(raw.len());
    for ip in raw {
        if !unique.contains(&ip) {
            unique.push(ip);
        }
    }
    Ok(unique)
}

fn tls_connector(options: &Options) -> Result<TlsConnector, CheckError> {
    let mut config = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|_| CheckError::Tls)?
        .with_root_certificates(Arc::clone(&options.roots))
        .with_no_client_auth();
    // No session state survives a check, and no application protocol is offered.
    config.resumption = Resumption::disabled();
    config.alpn_protocols.clear();
    config.enable_early_data = false;
    Ok(TlsConnector::from(Arc::new(config)))
}

/// Sends `close_notify` without any application data; failures are irrelevant
/// once the handshake has succeeded.
async fn close(mut stream: TlsStream<TcpStream>) {
    let _ = timeout(SHUTDOWN_TIMEOUT, stream.shutdown()).await;
}

#[cfg(test)]
mod tests;

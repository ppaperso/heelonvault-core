//! URL validation: extracts only the host and port, everything else is dropped.

use std::net::IpAddr;

use url::{Host, Url};

use crate::CheckError;

/// Upper bound on the accepted input length, to keep parsing bounded.
pub(crate) const MAX_URL_LEN: usize = 2048;

const DEFAULT_HTTPS_PORT: u16 = 443;

/// The only parts of the URL that ever reach the network layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetHost {
    /// Lower-cased, IDNA (punycode) normalized domain name.
    Domain(String),
    Ip(IpAddr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) host: TargetHost,
    pub(crate) port: u16,
}

pub(crate) fn parse_target(raw_url: &str) -> Result<Target, CheckError> {
    if raw_url.len() > MAX_URL_LEN {
        return Err(CheckError::InvalidUrl);
    }
    let url = Url::parse(raw_url).map_err(|_| CheckError::InvalidUrl)?;
    if url.scheme() != "https" {
        return Err(CheckError::HttpsRequired);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CheckError::CredentialsNotAllowed);
    }
    let port = url.port().unwrap_or(DEFAULT_HTTPS_PORT);
    if port == 0 {
        return Err(CheckError::InvalidUrl);
    }
    let host = match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.');
            if domain.is_empty() {
                return Err(CheckError::InvalidUrl);
            }
            if is_local_name(domain) {
                return Err(CheckError::NonPublicAddress);
            }
            TargetHost::Domain(domain.to_owned())
        }
        Some(Host::Ipv4(v4)) => TargetHost::Ip(IpAddr::V4(v4)),
        Some(Host::Ipv6(v6)) => TargetHost::Ip(IpAddr::V6(v6)),
        None => return Err(CheckError::InvalidUrl),
    };
    Ok(Target { host, port })
}

/// Names that are by definition local (RFC 6761 / RFC 6762 / RFC 8375), rejected
/// before any DNS query is made. Resolved addresses are still filtered afterwards.
fn is_local_name(domain: &str) -> bool {
    const LOCAL_NAMES: &[&str] = &["localhost", "local", "home.arpa", "internal"];
    LOCAL_NAMES.iter().any(|name| {
        domain == *name
            || domain
                .strip_suffix(name)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

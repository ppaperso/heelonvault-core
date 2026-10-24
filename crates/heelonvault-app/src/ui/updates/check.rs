use std::time::Duration;

use semver::Version;
use serde::Deserialize;

pub(super) const RELEASES: &str = "https://github.com/ppaperso/heelonvault-core/releases";
const ENDPOINT: &str = "https://api.github.com/repos/ppaperso/heelonvault-core/releases/latest";
const MAX_RESPONSE: usize = 64 * 1024;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

#[derive(Clone)]
pub(super) struct StableRelease {
    pub version: Version,
    tag: String,
}

impl StableRelease {
    pub fn from_tag(tag: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(tag.len() <= 128, "release tag too long");
        let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))?;
        anyhow::ensure!(version.pre.is_empty(), "prerelease tag");
        Ok(Self {
            version,
            tag: tag.to_string(),
        })
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn url(&self) -> String {
        format!("{RELEASES}/tag/{}", self.tag)
    }
}

fn parse_release(body: &[u8]) -> anyhow::Result<StableRelease> {
    let release: Release = serde_json::from_slice(body)?;
    anyhow::ensure!(
        !release.draft && !release.prerelease,
        "not a stable release"
    );
    StableRelease::from_tag(&release.tag_name)
}

pub(super) fn newer(latest: &Version, installed: &Version) -> bool {
    latest.cmp_precedence(installed).is_gt()
}

pub(super) async fn latest() -> anyhow::Result<StableRelease> {
    fetch(ENDPOINT, Duration::from_secs(8)).await
}

async fn fetch(endpoint: &str, timeout: Duration) -> anyhow::Result<StableRelease> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout.min(Duration::from_secs(4)))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("HeelonVault-Update-Check")
        .build()?;
    let mut response = client
        .get(endpoint)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?;
    anyhow::ensure!(
        response.status().is_success(),
        "release endpoint returned {}",
        response.status()
    );
    anyhow::ensure!(
        response
            .content_length()
            .is_none_or(|length| length <= MAX_RESPONSE as u64),
        "release response too large"
    );
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            body.len().saturating_add(chunk.len()) <= MAX_RESPONSE,
            "release response too large"
        );
        body.extend_from_slice(&chunk);
    }
    parse_release(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn stable_versions_only_and_semver_ordering() -> anyhow::Result<()> {
        for body in [
            br#"{"tag_name":"v2.2.0","draft":true,"prerelease":false}"#.as_slice(),
            br#"{"tag_name":"v2.2.0-rc.1","draft":false,"prerelease":false}"#,
            br#"{"tag_name":"v2.2.0","draft":false,"prerelease":true}"#,
            br#"{"tag_name":"not-a-version","draft":false,"prerelease":false}"#,
            br#"{}"#,
        ] {
            assert!(parse_release(body).is_err());
        }
        let installed = Version::parse("2.1.1")?;
        assert!(newer(&Version::parse("2.1.10")?, &installed));
        assert!(!newer(&installed, &installed));
        assert!(!newer(&Version::parse("2.1.0")?, &installed));
        assert!(!newer(&Version::parse("2.1.1+build")?, &installed));
        assert!(newer(&installed, &Version::parse("2.1.1-rc.1")?));
        assert_eq!(
            StableRelease::from_tag("v2.1.1")?.url(),
            format!("{RELEASES}/tag/v2.1.1")
        );
        assert_eq!(
            StableRelease::from_tag("2.1.1")?.url(),
            format!("{RELEASES}/tag/2.1.1")
        );
        for tag in [
            "../../other",
            "v2.1.1?redirect=evil",
            "v2.1.1#evil",
            "v2.1.1/evil",
        ] {
            assert!(StableRelease::from_tag(tag).is_err());
        }
        assert!(StableRelease::from_tag(&format!("v2.1.1+{}", "a".repeat(128))).is_err());
        Ok(())
    }

    #[test]
    fn bounded_http_and_untrusted_release_metadata() -> anyhow::Result<()> {
        tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        for (status, body, accepted) in [
            ("200 OK", r#"{"tag_name":"v2.2.0","draft":false,"prerelease":false,"html_url":"https://untrusted.invalid","body":"ignored"}"#.to_string(), true),
            ("302 Found", String::new(), false),
            ("403 Forbidden", "{}".to_string(), false),
            ("404 Not Found", "{}".to_string(), false),
            ("200 OK", "invalid json".to_string(), false),
            ("200 OK", " ".repeat(MAX_RESPONSE + 1), false),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let url = format!("http://{}", listener.local_addr()?);
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await?;
                let mut request = [0; 4096];
                let size = stream.read(&mut request).await?;
                let request = String::from_utf8_lossy(&request[..size]).to_ascii_lowercase();
                assert!(!request.contains("authorization:"));
                assert!(!request.contains("cookie:"));
                let header = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                stream.write_all(header.as_bytes()).await?;
                // The client may reject an oversized response before reading its body.
                let _ = stream.write_all(body.as_bytes()).await;
                Ok::<_, anyhow::Error>(())
            });
            assert_eq!(fetch(&url, Duration::from_secs(2)).await.is_ok(), accepted);
            server.await??;
        }
        Ok(())
        })
    }

    #[test]
    fn timeout_and_chunked_size_limit() -> anyhow::Result<()> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                for stall in [true, false] {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
                    let url = format!("http://{}", listener.local_addr()?);
                    let server = tokio::spawn(async move {
                        let (mut stream, _) = listener.accept().await?;
                        let mut request = [0; 4096];
                        let size = stream.read(&mut request).await?;
                        anyhow::ensure!(size > 0, "empty test request");
                        if stall {
                            tokio::time::sleep(Duration::from_millis(150)).await;
                        } else {
                            stream
                                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                                .await?;
                            let chunk = " ".repeat(MAX_RESPONSE + 1);
                            let header = format!("{:x}\r\n", chunk.len());
                            stream.write_all(header.as_bytes()).await?;
                            let _ = stream.write_all(chunk.as_bytes()).await;
                        }
                        Ok::<_, anyhow::Error>(())
                    });
                    let error = fetch(&url, Duration::from_millis(100))
                        .await
                        .err()
                        .ok_or_else(|| anyhow::anyhow!("expected a bounded HTTP failure"))?;
                    if stall {
                        assert!(
                            error
                                .downcast_ref::<reqwest::Error>()
                                .is_some_and(reqwest::Error::is_timeout)
                        );
                    } else {
                        assert!(error.to_string().contains("too large"));
                    }
                    server.await??;
                }
                Ok(())
            })
    }
}

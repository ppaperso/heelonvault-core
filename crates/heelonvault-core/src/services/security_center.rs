//! Local-only diagnostics. Reports contain neither plaintext values nor password fingerprints.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use secrecy::{ExposeSecret, SecretBox};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::errors::AppError;
use crate::models::{SecretItem, SecretType};
use crate::services::crypto_service::{CryptoService, CryptoServiceImpl};
use crate::services::secret_service::{SecretService, decode_secret_payload};
use crate::services::vault_service::VaultService;

const APP_TERMS: &[&str] = &[
    "heelon",
    "vault",
    "heelonvault",
    "test",
    "azerty",
    "qwerty",
    "qwertz",
    "123456",
    "password",
    "motdepasse",
];

#[derive(Hash, PartialEq, Eq)]
struct Fingerprint([u8; 32]);

impl Drop for Fingerprint {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Shared estimator for stored passwords and the editor. Master-password acceptance is separate.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PasswordAssessment {
    /// Final 0–4 score: the estimator score bounded by [`Self::policy_cap`].
    pub score: u8,
    pub estimator_score: u8,
    /// Upper bound set by the length/character-class policy, whatever the estimator says.
    pub policy_cap: u8,
    /// Untranslated zxcvbn feedback (English); callers localize it before display.
    pub feedback: Option<String>,
}

/// Scores below this value are reported as weak.
pub const WEAK_SCORE_THRESHOLD: u8 = 3;

/// Length/character-class cap (ANSSI-aligned), independent of the estimator.
fn policy_cap(password: &str) -> u8 {
    let categories = [
        password.chars().any(char::is_uppercase),
        password.chars().any(char::is_lowercase),
        password.chars().any(|c| c.is_ascii_digit()),
        password.chars().any(|c| !c.is_alphanumeric()),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    let len = password.chars().count();
    if len < 12 {
        1
    } else if len < 14 {
        if categories >= 4 { 3 } else { 2 }
    } else if len == 14 {
        if categories >= 3 { 3 } else { 2 }
    } else if categories >= 3 {
        4
    } else {
        2
    }
}

pub fn assess_password(password: &str, username: &str) -> PasswordAssessment {
    let mut inputs = APP_TERMS.to_vec();
    if !username.is_empty() {
        inputs.push(username);
    }
    let estimate = zxcvbn::zxcvbn(password, &inputs);
    let estimator_score = estimate.score() as u8;
    let cap = policy_cap(password);
    let feedback = estimate.feedback().as_ref().and_then(|feedback| {
        feedback
            .warning()
            .map(|warning| warning.to_string())
            .or_else(|| feedback.suggestions().first().map(ToString::to_string))
    });
    PasswordAssessment {
        score: estimator_score.min(cap),
        estimator_score,
        policy_cap: cap,
        feedback,
    }
}

/// Same verdict as `assess_password(password, "").score < WEAK_SCORE_THRESHOLD`, but skips the
/// estimator (about 0.2–1.5 ms per password in release builds) when the policy cap alone
/// already makes the password weak. Use it for bulk scans.
pub fn is_weak_password(password: &str) -> bool {
    policy_cap(password) < WEAK_SCORE_THRESHOLD
        || assess_password(password, "").score < WEAK_SCORE_THRESHOLD
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UrlStatus {
    Empty,
    Invalid,
    UnsupportedScheme,
    Credentials,
    Http,
    LocalHost,
    InternationalDomain,
    HttpsUnverified,
}

impl UrlStatus {
    pub fn needs_attention(self) -> bool {
        !matches!(self, Self::Empty | Self::HttpsUnverified)
    }
}

/// Syntax only: HTTPS here does not mean a certificate or reputation was checked.
pub fn assess_url(raw: &str) -> UrlStatus {
    if raw.trim().is_empty() {
        return UrlStatus::Empty;
    }
    let Ok(url) = Url::parse(raw.trim()) else {
        return UrlStatus::Invalid;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return UrlStatus::UnsupportedScheme;
    }
    let Some(host) = url.host_str() else {
        return UrlStatus::Invalid;
    };
    if !url.username().is_empty() || url.password().is_some() {
        return UrlStatus::Credentials;
    }
    if !host.contains('.')
        || host.ends_with(".local")
        || host.ends_with(".localhost")
        || host.ends_with(".internal")
        || host.ends_with(".home.arpa")
        || url
            .host()
            .is_some_and(|host| !matches!(host, url::Host::Domain(_)))
    {
        return UrlStatus::LocalHost;
    }
    if url.scheme() == "http" {
        return UrlStatus::Http;
    }
    if host.split('.').any(|label| label.starts_with("xn--")) {
        return UrlStatus::InternationalDomain;
    }
    UrlStatus::HttpsUnverified
}

/// Typed findings, not proof of compromise or regulatory non-compliance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Finding {
    WeakPassword,
    ReusedPassword,
    Expired,
    ExpiringSoon,
    InvalidExpiry,
    Url(UrlStatus),
    Unreadable,
    InvalidMetadata,
}

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct SecurityEntry {
    pub secret_id: Uuid,
    pub vault_id: Uuid,
    pub title: String,
    pub vault_name: String,
    pub findings: Vec<Finding>,
    /// Ephemeral report-local group number; never a password hash.
    pub reuse_group: Option<usize>,
}

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct SecurityReport {
    pub entries: Vec<SecurityEntry>,
    pub vaults_analyzed: usize,
    pub unavailable_vaults: usize,
    pub analyzed: usize,
    pub unreadable: usize,
}

impl SecurityReport {
    pub fn affected(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| !entry.findings.is_empty())
            .count()
    }

    pub fn is_complete(&self) -> bool {
        self.unavailable_vaults == 0
            && self.unreadable == 0
            && self.entries.iter().all(|entry| {
                !entry.findings.iter().any(|finding| {
                    matches!(finding, Finding::InvalidExpiry | Finding::InvalidMetadata)
                })
            })
    }
}

/// Cancellation is checked between operations; callers must invalidate pending UI results on lock.
#[derive(Debug, Default)]
pub struct ScanControl {
    cancelled: AtomicBool,
    processed: AtomicUsize,
}

impl ScanControl {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Items analyzed so far; readable from another thread for progress display.
    pub fn processed(&self) -> usize {
        self.processed.load(Ordering::Acquire)
    }

    fn check(&self) -> Result<(), AppError> {
        if self.is_cancelled() {
            Err(AppError::Validation(
                "security analysis cancelled".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

fn expiry_finding(raw: &str, now: OffsetDateTime) -> Option<Finding> {
    match OffsetDateTime::parse(raw, &Rfc3339) {
        Ok(expiry) if expiry <= now => Some(Finding::Expired),
        Ok(expiry) if expiry <= now + Duration::days(30) => Some(Finding::ExpiringSoon),
        Ok(_) => None,
        Err(_) => Some(Finding::InvalidExpiry),
    }
}

fn metadata_findings(item: &SecretItem, now: OffsetDateTime) -> Vec<Finding> {
    let mut findings = Vec::new();
    if let Some(raw) = item.expires_at.as_deref().filter(|raw| !raw.is_empty())
        && let Some(finding) = expiry_finding(raw, now)
    {
        findings.push(finding);
    }
    if let Some(raw) = &item.metadata_json {
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(value) if value.is_object() => {
                if let Some(raw_url) = value.get("url") {
                    match raw_url.as_str() {
                        Some(raw) => {
                            let status = assess_url(raw);
                            if status.needs_attention() {
                                findings.push(Finding::Url(status));
                            }
                        }
                        None => findings.push(Finding::InvalidMetadata),
                    }
                }
            }
            _ => findings.push(Finding::InvalidMetadata),
        }
    }
    findings
}

/// Scan accessible vaults with one item query per vault, decrypting only password values.
/// Ciphertext is taken from the batch; no per-item repository re-read is needed.
pub async fn analyze<TSecret, TVault>(
    secrets: &TSecret,
    vaults: &TVault,
    user_id: Uuid,
    master_key: SecretBox<Vec<u8>>,
    now: OffsetDateTime,
    control: &ScanControl,
) -> Result<SecurityReport, AppError>
where
    TSecret: SecretService + Sync,
    TVault: VaultService + Sync,
{
    control.check()?;
    let mut report = SecurityReport::default();
    let mut fingerprints: HashMap<Fingerprint, (Vec<usize>, bool)> = HashMap::new();
    let crypto = CryptoServiceImpl::default();
    for vault in vaults.list_user_vaults(user_id).await? {
        control.check()?;
        let key = match vaults
            .open_vault_for_user(
                user_id,
                vault.id,
                SecretBox::new(Box::new(master_key.expose_secret().clone())),
            )
            .await
        {
            Ok(key) => key,
            Err(_) => {
                report.unavailable_vaults += 1;
                continue;
            }
        };
        let items = match secrets.list_by_vault(vault.id).await {
            Ok(items) => items,
            Err(_) => {
                report.unavailable_vaults += 1;
                continue;
            }
        };
        report.vaults_analyzed += 1;
        for item in items {
            control.check()?;
            let mut findings = metadata_findings(&item, now);
            if matches!(item.secret_type, SecretType::Password) {
                let decrypted = match decode_secret_payload(&item.secret_blob) {
                    Ok(payload) => crypto.decrypt(&payload, &key).await,
                    Err(error) => Err(error),
                };
                match decrypted {
                    Ok(value) => match std::str::from_utf8(value.expose_secret()) {
                        Ok(text) if !text.is_empty() => {
                            let fingerprint = Fingerprint(Sha256::digest(text.as_bytes()).into());
                            let (indices, weak) = fingerprints
                                .entry(fingerprint)
                                .or_insert_with(|| (Vec::new(), is_weak_password(text)));
                            if *weak {
                                findings.push(Finding::WeakPassword);
                            }
                            indices.push(report.entries.len());
                        }
                        _ => {
                            findings.push(Finding::Unreadable);
                            report.unreadable += 1;
                        }
                    },
                    Err(_) => {
                        findings.push(Finding::Unreadable);
                        report.unreadable += 1;
                    }
                }
            }
            report.entries.push(SecurityEntry {
                secret_id: item.id,
                vault_id: item.vault_id,
                title: item.title.unwrap_or_default(),
                vault_name: vault.name.clone(),
                findings,
                reuse_group: None,
            });
            report.analyzed += 1;
            control.processed.store(report.analyzed, Ordering::Release);
        }
    }
    control.check()?;
    // Assign stable group numbers by report order, not randomized HashMap iteration.
    let mut groups: Vec<_> = fingerprints
        .values()
        .map(|(indices, _)| indices)
        .filter(|indices| indices.len() > 1)
        .collect();
    groups.sort_by_key(|indices| indices[0]);
    for (group, indices) in groups.into_iter().enumerate() {
        for &index in indices {
            report.entries[index].findings.push(Finding::ReusedPassword);
            report.entries[index].reuse_group = Some(group);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_url_checks_never_claim_tls_validation() {
        for (raw, expected) in [
            ("", UrlStatus::Empty),
            ("not a url", UrlStatus::Invalid),
            ("javascript:alert(1)", UrlStatus::UnsupportedScheme),
            ("https://user:pass@example.com", UrlStatus::Credentials),
            ("http://example.com", UrlStatus::Http),
            ("https://intranet", UrlStatus::LocalHost),
            ("https://127.0.0.1", UrlStatus::LocalHost),
            ("https://[::1]", UrlStatus::LocalHost),
            ("https://router.home.arpa", UrlStatus::LocalHost),
            ("https://xn--bcher-kva.de", UrlStatus::InternationalDomain),
            (
                "https://example.com/private?token=secret",
                UrlStatus::HttpsUnverified,
            ),
        ] {
            assert_eq!(assess_url(raw), expected);
        }
    }

    #[test]
    fn common_complex_password_is_still_weak() {
        assert!(assess_password("Password123!", "").score < 3);
        assert_eq!(assess_password("", "").score, 0);
        assert!(assess_password("eW8!rK2#qL7$vN4@zT9", "").score >= 3);
    }

    #[test]
    fn fast_weak_verdict_matches_full_assessment() {
        for password in [
            "",
            "short",
            "Password123!",
            "onlylowercaseletters",
            "Correct-Horse-42",
            "Éléphant-Rose-2026",
            "eW8!rK2#qL7$vN4@zT9",
            " leading and trailing ",
            "àÉ8!xyz",
        ] {
            assert_eq!(
                is_weak_password(password),
                assess_password(password, "").score < WEAK_SCORE_THRESHOLD,
                "{password:?}"
            );
        }
    }

    #[test]
    fn expiry_boundaries_and_invalid_dates_are_explicit() -> Result<(), Box<dyn std::error::Error>>
    {
        let now = OffsetDateTime::parse("2026-10-18T08:00:00Z", &Rfc3339)?;
        assert_eq!(
            expiry_finding("2026-10-18T08:00:00Z", now),
            Some(Finding::Expired)
        );
        assert_eq!(
            expiry_finding("2026-11-17T08:00:00Z", now),
            Some(Finding::ExpiringSoon)
        );
        assert_eq!(expiry_finding("2026-11-17T08:00:01Z", now), None);
        assert_eq!(
            expiry_finding("tomorrow", now),
            Some(Finding::InvalidExpiry)
        );
        Ok(())
    }

    #[test]
    fn cancellation_and_coverage_are_not_success_shaped() {
        let control = ScanControl::default();
        control.cancel();
        assert!(control.check().is_err());
        let report = SecurityReport {
            unavailable_vaults: 1,
            ..SecurityReport::default()
        };
        assert!(!report.is_complete());
    }
}

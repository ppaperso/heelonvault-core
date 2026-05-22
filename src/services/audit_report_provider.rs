use thiserror::Error;

/// Errors that can occur when generating an audit report.
/// Defined here (not in `audit_report_service`) so that the error type is
/// available to the `AuditReportProvider` trait in Community builds.
#[derive(Debug, Error)]
pub enum ReportError {
    #[error("a Professional or Enterprise license is required")]
    LicenseRequired,
    #[error("feature not available in this edition: {0}")]
    FeatureNotAvailable(&'static str),
    #[error("audit signing key is missing")]
    SigningKeyMissing,
    #[error("downloads directory not found")]
    DownloadsDirectoryNotFound,
    #[error("failed to initialize PDF fonts: {0}")]
    FontInitialization(String),
    #[error("failed to load audit log entries: {0}")]
    AuditData(String),
    #[error("failed to sign audit report: {0}")]
    Signature(String),
    #[error("failed to render or write PDF: {0}")]
    PdfWrite(String),
}

/// Metadata returned after a successful audit report generation.
pub struct GeneratedAuditReport {
    pub path: String,
    pub hash_hex: String,
}

impl GeneratedAuditReport {
    pub fn hash_prefix(&self) -> &str {
        let prefix_len = self.hash_hex.len().min(12);
        &self.hash_hex[..prefix_len]
    }
}

/// Abstraction over audit-report generation.  Always compiled so Core services
/// can hold a `Box<dyn AuditReportProvider>` without the `premium` feature.
pub trait AuditReportProvider: Send + Sync + 'static {
    fn generate_report(
        &self,
        customer_name: &str,
        days: i64,
    ) -> Result<GeneratedAuditReport, ReportError>;
}

/// Community stub: always returns [`ReportError::FeatureNotAvailable`].
pub struct CommunityAuditReportProvider;

impl AuditReportProvider for CommunityAuditReportProvider {
    fn generate_report(
        &self,
        _customer_name: &str,
        _days: i64,
    ) -> Result<GeneratedAuditReport, ReportError> {
        Err(ReportError::FeatureNotAvailable(
            "feature-name-audit-report",
        ))
    }
}

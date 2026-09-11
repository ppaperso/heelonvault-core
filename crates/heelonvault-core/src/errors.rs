use std::fmt;
use thiserror::Error;

/// Semantic reason for an authorization denial.
/// Used as the payload of [`AppError::Authorization`] so the UI can
/// display a localized message without parsing an English string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessDeniedReason {
    /// The action requires admin role.
    AdminRequired,
    /// The action requires the team-leader role within the team.
    TeamLeaderRequired,
    /// The action requires being a member of the team.
    TeamMembershipRequired,
    /// The user does not have access to the vault.
    VaultAccessDenied,
    /// The user has read-only access; a write operation was attempted.
    VaultWriteDenied,
    /// The user lacks admin rights on the vault (share/revoke/rotate/delete).
    VaultAdminRequired,
    /// Creating a secret in a shared vault requires vault-admin rights.
    VaultSharedCreateDenied,
    /// Credentials supplied were rejected.
    InvalidCredentials,
    /// The current password must be supplied to perform this profile change.
    PasswordRequiredForChange,
    /// The TOTP setup verification code was wrong.
    InvalidTotpCode,
    /// A user tried to read another user's audit log without admin rights.
    AuditCrossUserDenied,
    /// A catch-all for actions that no specific rule covers.
    Unauthorized,
}

impl fmt::Display for AccessDeniedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdminRequired => write!(f, "admin role required"),
            Self::TeamLeaderRequired => write!(f, "team leader role required"),
            Self::TeamMembershipRequired => write!(f, "team membership required"),
            Self::VaultAccessDenied => write!(f, "vault access denied for this user"),
            Self::VaultWriteDenied => write!(f, "vault write denied for this user"),
            Self::VaultAdminRequired => write!(f, "vault administration requires admin permission"),
            Self::VaultSharedCreateDenied => {
                write!(f, "creating secrets in shared vault requires admin role")
            }
            Self::InvalidCredentials => write!(f, "invalid credentials"),
            Self::PasswordRequiredForChange => {
                write!(f, "current password required for this change")
            }
            Self::InvalidTotpCode => write!(f, "invalid TOTP setup code"),
            Self::AuditCrossUserDenied => write!(
                f,
                "insufficient permissions to view another user's audit log"
            ),
            Self::Unauthorized => write!(f, "unauthorized action"),
        }
    }
}

/// Why a backup restore or an account re-key failed, so the UI can localize it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryFailure {
    InvalidPhrase,
    WrongPhraseOrAlteredFile,
    NotABackup,
    UnsupportedBackupVersion(u16),
    NoAccount,
    MultipleAccounts,
    /// A vault does not open with the account key; nothing was written.
    InconsistentKeyMaterial,
    /// The account key is not sealed with the recovery phrase in this backup.
    RecoveryKeyMissing,
}

impl fmt::Display for RecoveryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhrase => write!(f, "the recovery phrase is not a valid BIP39 phrase"),
            Self::WrongPhraseOrAlteredFile => {
                write!(f, "wrong recovery phrase or altered backup file")
            }
            Self::NotABackup => write!(f, "the file is not a HeelonVault backup"),
            Self::UnsupportedBackupVersion(version) => {
                write!(f, "unsupported backup format version {version}")
            }
            Self::NoAccount => write!(f, "the backup contains no account"),
            Self::MultipleAccounts => write!(
                f,
                "password reset from a backup requires a single-account vault"
            ),
            Self::InconsistentKeyMaterial => write!(
                f,
                "a vault does not open with the account key; nothing was modified"
            ),
            Self::RecoveryKeyMissing => {
                write!(f, "the backup holds no recovery key for this account")
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("initialization required: {0}")]
    InitializationRequired(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("authorization error: {0}")]
    Authorization(AccessDeniedReason),
    #[error("recovery error: {0}")]
    Recovery(RecoveryFailure),
    /// Accounts of an account-key vault cannot share vaults yet, so it holds a single account.
    #[error("this vault uses an account key and holds a single account")]
    SingleAccountVault,
    #[error("feature not available in this edition: {0}")]
    FeatureNotAvailable(&'static str),
    #[error("the license has expired")]
    LicenseExpired,
    #[error("shutdown in progress")]
    ShutdownInProgress,
    #[error("internal error")]
    Internal,
}

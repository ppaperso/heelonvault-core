use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the license tier and mode of HeelonVault.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseTier {
    /// Community (free) edition - unlimited usage, no signature required
    Community,
    /// Professional edition - signed license, limited slots, expiration date
    Professional,
}

impl fmt::Display for LicenseTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Community => write!(f, "Community"),
            Self::Professional => write!(f, "Professional"),
        }
    }
}

/// HeelonVault license metadata.
/// Must be signed with ed25519 private key by HEELONYS.
/// Signature prevents tampering with customer_name, slots_count, or expiration_date.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct License {
    /// Unique license identifier (UUID v4)
    pub id: String,

    /// Customer organization name (for audit trail)
    pub customer_name: String,

    /// Maximum number of sites (machines) authorized to run HeelonVault
    pub slots_count: u32,

    /// License expiration timestamp (ISO 8601 string)
    pub expiration_date: String,

    /// Features enabled (reserved for future extension)
    pub features: Vec<String>,

    /// License tier
    pub tier: LicenseTier,
}

impl License {
    /// Check if license is still valid (not expired).
    pub fn is_valid(&self) -> bool {
        match chrono::DateTime::parse_from_rfc3339(&self.expiration_date) {
            Ok(expiration) => chrono::Utc::now() <= expiration.with_timezone(&chrono::Utc),
            Err(_) => false,
        }
    }

    /// Check if a given number of slots is within the license limit.
    pub fn has_capacity(&self, current_slots: u32) -> bool {
        current_slots < self.slots_count
    }
}

/// Signed license bundle: serialized license + ed25519 signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedLicense {
    /// JSON-encoded license
    pub payload: String,

    /// Ed25519 signature (hex-encoded, 64 bytes = 128 hex chars)
    pub signature: String,
}

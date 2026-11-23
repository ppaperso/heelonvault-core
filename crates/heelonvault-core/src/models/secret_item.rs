use secrecy::SecretBox;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecretType {
    Password,
    ApiToken,
    SshKey,
    SecureDocument,
}

impl SecretType {
    pub fn from_dropdown_index(index: u32) -> Option<Self> {
        match index {
            0 => Some(Self::Password),
            1 => Some(Self::ApiToken),
            2 => Some(Self::SshKey),
            3 => Some(Self::SecureDocument),
            _ => None,
        }
    }

    pub fn dropdown_index(self) -> u32 {
        match self {
            Self::Password => 0,
            Self::ApiToken => 1,
            Self::SshKey => 2,
            Self::SecureDocument => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BlobStorage {
    Inline,
    File,
}

#[derive(Debug)]
pub struct SecretItem {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub secret_type: SecretType,
    pub title: Option<String>,
    pub metadata_json: Option<String>,
    pub tags: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub usage_count: u32,
    pub blob_storage: BlobStorage,
    pub secret_blob: SecretBox<Vec<u8>>,
    /// Populated only when the item comes from the trash query (soft-deleted).
    pub deleted_at: Option<String>,
}

/// Kind of a password item, stored in `metadata_json` under [`PasswordKind::METADATA_KEY`].
///
/// Passphrases keep [`SecretType::Password`]: they have the same fields, and older versions
/// read them as plain passwords. A missing or unknown value means [`PasswordKind::Password`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PasswordKind {
    #[default]
    Password,
    Passphrase,
}

impl PasswordKind {
    /// Metadata key holding the kind.
    pub const METADATA_KEY: &'static str = "password_kind";
    /// Metadata key holding the non-secret generator recipe, when the value was generated.
    pub const RECIPE_METADATA_KEY: &'static str = "generator_recipe";

    pub fn as_metadata_value(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Passphrase => "passphrase",
        }
    }

    /// Reads the kind from parsed item metadata; missing or unknown values are passwords.
    pub fn from_metadata(metadata: &serde_json::Value) -> Self {
        match metadata
            .get(Self::METADATA_KEY)
            .and_then(serde_json::Value::as_str)
        {
            Some("passphrase") => Self::Passphrase,
            _ => Self::Password,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PasswordKind;

    #[test]
    fn password_kind_defaults_to_password() -> Result<(), serde_json::Error> {
        for raw in [
            r#"{}"#,
            r#"{"password_kind":"unknown"}"#,
            r#"{"password_kind":42}"#,
            r#"{"password_kind":"password"}"#,
        ] {
            let value: serde_json::Value = serde_json::from_str(raw)?;
            assert_eq!(
                PasswordKind::from_metadata(&value),
                PasswordKind::Password,
                "{raw}"
            );
        }
        let value: serde_json::Value = serde_json::from_str(r#"{"password_kind":"passphrase"}"#)?;
        assert_eq!(
            PasswordKind::from_metadata(&value),
            PasswordKind::Passphrase
        );
        assert_eq!(PasswordKind::Passphrase.as_metadata_value(), "passphrase");
        Ok(())
    }
}

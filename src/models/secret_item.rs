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

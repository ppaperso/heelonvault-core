use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Vault {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultShareRole {
    Read,
    Write,
    Admin,
}

impl VaultShareRole {
    pub fn to_db_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Admin => "admin",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }

    pub fn can_write(self) -> bool {
        matches!(self, Self::Write | Self::Admin)
    }

    pub fn can_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultAccessKind {
    Owner,
    DirectShare,
    TeamShare,
}

impl VaultAccessKind {
    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw {
            "owner" => Some(Self::Owner),
            "direct_share" => Some(Self::DirectShare),
            "team_share" => Some(Self::TeamShare),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccessibleVault {
    pub vault: Vault,
    pub role: VaultShareRole,
    pub access_kind: VaultAccessKind,
    pub vault_key_version: i64,
}

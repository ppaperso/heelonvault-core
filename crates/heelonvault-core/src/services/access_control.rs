use crate::errors::{AccessDeniedReason, AppError};
use crate::models::{TeamMemberRole, User, UserRole, VaultShareRole};

#[derive(Debug, Clone, Copy)]
pub enum Action {
    AdminManageUsers,
    TeamManageMembers,
    TeamReadMembers,
    VaultCreate,
    VaultOpen,
    VaultWrite,
    VaultList,
    VaultDelete,
    VaultShare,
    VaultRevoke,
    VaultRotate,
    AuditRead,
    AuditWrite,
    BackupExport,
    BackupRestore,
}

#[derive(Debug, Clone)]
pub enum Resource {
    Global,
    Team {
        requester_role: Option<TeamMemberRole>,
    },
    Vault {
        is_owner: bool,
        has_direct_share: bool,
        has_team_share: bool,
        share_role: Option<VaultShareRole>,
    },
}

pub fn check_permission(user: &User, action: Action, resource: &Resource) -> Result<(), AppError> {
    if matches!(user.role, UserRole::Admin) {
        return Ok(());
    }

    match (action, resource) {
        (Action::AdminManageUsers, _) => {
            Err(AppError::Authorization(AccessDeniedReason::AdminRequired))
        }
        (Action::AuditRead, _) => Err(AppError::Authorization(AccessDeniedReason::AdminRequired)),
        (Action::AuditWrite, _) => Ok(()),
        (Action::TeamManageMembers, Resource::Team { requester_role }) => {
            if matches!(requester_role, Some(TeamMemberRole::Leader)) {
                Ok(())
            } else {
                Err(AppError::Authorization(
                    AccessDeniedReason::TeamLeaderRequired,
                ))
            }
        }
        (Action::TeamReadMembers, Resource::Team { requester_role }) => {
            if requester_role.is_some() {
                Ok(())
            } else {
                Err(AppError::Authorization(
                    AccessDeniedReason::TeamMembershipRequired,
                ))
            }
        }
        (Action::VaultCreate, Resource::Global) => Ok(()),
        (Action::VaultList, Resource::Global) => Ok(()),
        (
            Action::VaultDelete,
            Resource::Vault {
                is_owner,
                has_direct_share: _,
                has_team_share: _,
                share_role,
            },
        ) => {
            if *is_owner || share_role.is_some_and(|role| role.can_admin()) {
                Ok(())
            } else {
                Err(AppError::Authorization(
                    AccessDeniedReason::VaultAdminRequired,
                ))
            }
        }
        (
            Action::VaultWrite,
            Resource::Vault {
                is_owner,
                has_direct_share,
                has_team_share,
                share_role,
            },
        ) => {
            let has_access = *is_owner || *has_direct_share || *has_team_share;
            if has_access && (*is_owner || share_role.is_some_and(|role| role.can_write())) {
                Ok(())
            } else {
                Err(AppError::Authorization(
                    AccessDeniedReason::VaultWriteDenied,
                ))
            }
        }
        (
            Action::VaultOpen,
            Resource::Vault {
                is_owner,
                has_direct_share,
                has_team_share,
                share_role: _,
            },
        ) => {
            if *is_owner || *has_direct_share || *has_team_share {
                Ok(())
            } else {
                Err(AppError::Authorization(
                    AccessDeniedReason::VaultAccessDenied,
                ))
            }
        }
        (
            Action::VaultShare | Action::VaultRevoke | Action::VaultRotate,
            Resource::Vault {
                is_owner,
                has_direct_share,
                has_team_share,
                share_role,
            },
        ) => {
            let has_access = *is_owner || *has_direct_share || *has_team_share;
            if has_access && (*is_owner || share_role.is_some_and(|role| role.can_admin())) {
                Ok(())
            } else {
                Err(AppError::Authorization(
                    AccessDeniedReason::VaultAdminRequired,
                ))
            }
        }
        (Action::BackupExport, _) => {
            Err(AppError::Authorization(AccessDeniedReason::AdminRequired))
        }
        (Action::BackupRestore, _) => {
            Err(AppError::Authorization(AccessDeniedReason::AdminRequired))
        }
        _ => Err(AppError::Authorization(AccessDeniedReason::Unauthorized)),
    }
}

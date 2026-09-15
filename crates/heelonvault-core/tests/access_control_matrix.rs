//! Non-regression of the permission model: `check_permission` is compared, for every action ×
//! resource × role combination, with an independent statement of the expected policy. Any
//! change to the matrix must be made in both places, on purpose.

use heelonvault_core::errors::{AccessDeniedReason, AppError};
use heelonvault_core::models::{TeamMemberRole, User, UserRole, VaultShareRole};
use heelonvault_core::services::access_control::{Action, Resource, check_permission};
use uuid::Uuid;

const ACTIONS: [Action; 15] = [
    Action::AdminManageUsers,
    Action::TeamManageMembers,
    Action::TeamReadMembers,
    Action::VaultCreate,
    Action::VaultOpen,
    Action::VaultWrite,
    Action::VaultList,
    Action::VaultDelete,
    Action::VaultShare,
    Action::VaultRevoke,
    Action::VaultRotate,
    Action::AuditRead,
    Action::AuditWrite,
    Action::BackupExport,
    Action::BackupRestore,
];

fn user(role: UserRole) -> User {
    User {
        id: Uuid::new_v4(),
        username: "subject".to_string(),
        role,
        email: None,
        display_name: None,
        preferred_language: "fr".to_string(),
        show_passwords_in_edit: false,
        updated_at: None,
    }
}

fn resources() -> Vec<Resource> {
    let mut all = vec![Resource::Global];
    for requester_role in [
        None,
        Some(TeamMemberRole::Member),
        Some(TeamMemberRole::Leader),
    ] {
        all.push(Resource::Team { requester_role });
    }
    let roles = [
        None,
        Some(VaultShareRole::Read),
        Some(VaultShareRole::Write),
        Some(VaultShareRole::Admin),
    ];
    for is_owner in [false, true] {
        for has_direct_share in [false, true] {
            for has_team_share in [false, true] {
                for share_role in roles {
                    all.push(Resource::Vault {
                        is_owner,
                        has_direct_share,
                        has_team_share,
                        share_role,
                    });
                }
            }
        }
    }
    all
}

/// The policy, written independently from the implementation. `None` means allowed.
fn expected_for_non_admin(action: Action, resource: &Resource) -> Option<AccessDeniedReason> {
    use AccessDeniedReason::*;
    match action {
        Action::AdminManageUsers
        | Action::AuditRead
        | Action::BackupExport
        | Action::BackupRestore => Some(AdminRequired),
        Action::AuditWrite => None,
        Action::VaultCreate | Action::VaultList => match resource {
            Resource::Global => None,
            _ => Some(Unauthorized),
        },
        Action::TeamManageMembers => match resource {
            Resource::Team {
                requester_role: Some(TeamMemberRole::Leader),
            } => None,
            Resource::Team { .. } => Some(TeamLeaderRequired),
            _ => Some(Unauthorized),
        },
        Action::TeamReadMembers => match resource {
            Resource::Team {
                requester_role: Some(_),
            } => None,
            Resource::Team { .. } => Some(TeamMembershipRequired),
            _ => Some(Unauthorized),
        },
        Action::VaultOpen
        | Action::VaultWrite
        | Action::VaultDelete
        | Action::VaultShare
        | Action::VaultRevoke
        | Action::VaultRotate => {
            let Resource::Vault {
                is_owner,
                has_direct_share,
                has_team_share,
                share_role,
            } = resource
            else {
                return Some(Unauthorized);
            };
            // A role never grants anything without an actual access path to the vault.
            let has_access = *is_owner || *has_direct_share || *has_team_share;
            let can_write = *is_owner || share_role.is_some_and(VaultShareRole::can_write);
            let can_admin = *is_owner || share_role.is_some_and(VaultShareRole::can_admin);
            match action {
                Action::VaultOpen => (!has_access).then_some(VaultAccessDenied),
                Action::VaultWrite => (!(has_access && can_write)).then_some(VaultWriteDenied),
                _ => (!(has_access && can_admin)).then_some(VaultAdminRequired),
            }
        }
    }
}

#[test]
fn every_non_admin_decision_matches_the_policy() {
    let subject = user(UserRole::User);
    let mut mismatches = Vec::new();

    for action in ACTIONS {
        for resource in resources() {
            let actual = match check_permission(&subject, action, &resource) {
                Ok(()) => None,
                Err(AppError::Authorization(reason)) => Some(reason),
                Err(other) => panic!("unexpected error {other:?} for {action:?} on {resource:?}"),
            };
            let expected = expected_for_non_admin(action, &resource);
            if actual != expected {
                mismatches.push(format!(
                    "{action:?} on {resource:?}: expected {expected:?}, got {actual:?}"
                ));
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "{} permission decision(s) diverge from the policy:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn an_admin_is_allowed_everywhere_but_crypto_still_gates_vault_contents() {
    // Authorization lets an admin through; reading a vault still requires its key envelope,
    // which only the owner and explicit share recipients hold (see full_decryption_chain.rs).
    let admin = user(UserRole::Admin);
    for action in ACTIONS {
        for resource in resources() {
            assert!(
                check_permission(&admin, action, &resource).is_ok(),
                "admin denied {action:?} on {resource:?}"
            );
        }
    }
}

#[test]
fn a_read_only_share_can_open_but_never_modify_or_administer() {
    let subject = user(UserRole::User);
    let read_share = Resource::Vault {
        is_owner: false,
        has_direct_share: true,
        has_team_share: false,
        share_role: Some(VaultShareRole::Read),
    };

    assert!(check_permission(&subject, Action::VaultOpen, &read_share).is_ok());
    for action in [
        Action::VaultWrite,
        Action::VaultDelete,
        Action::VaultShare,
        Action::VaultRevoke,
        Action::VaultRotate,
    ] {
        assert!(
            check_permission(&subject, action, &read_share).is_err(),
            "a read-only share must not allow {action:?}"
        );
    }
}

//! What the signed-in user may see and do, from two independent facts: the edition granted by
//! the license (runtime, not the `premium` compile feature, which is on in every shipped binary)
//! and the account role. Every UI entry point to a Professional feature asks this type, so no
//! page can expose it on its own. The premium services enforce the same rule (`ProfessionalGate`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edition {
    Community,
    Professional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entitlements {
    pub edition: Edition,
    pub is_admin: bool,
}

impl Entitlements {
    pub fn new(edition: Edition, is_admin: bool) -> Self {
        Self { edition, is_admin }
    }

    /// User accounts administration (create, roles, password reset).
    pub fn can_manage_users(self) -> bool {
        self.is_admin && self.edition == Edition::Professional
    }

    /// Teams and shared vaults.
    pub fn can_manage_teams(self) -> bool {
        self.is_admin && self.edition == Edition::Professional
    }

    /// Encrypted `.hvb` export of the whole database: admin role, any edition.
    pub fn can_export_backup(self) -> bool {
        self.is_admin
    }

    /// An admin on Community sees what the Professional edition adds, without any action.
    pub fn shows_professional_offer(self) -> bool {
        self.is_admin && self.edition == Edition::Community
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn professional_features_need_both_the_license_and_the_admin_role() {
        let matrix = [
            (Edition::Community, false, false, false, false),
            (Edition::Community, true, false, true, true),
            (Edition::Professional, false, false, false, false),
            (Edition::Professional, true, true, true, false),
        ];
        for (edition, is_admin, manage, export, offer) in matrix {
            let rights = Entitlements::new(edition, is_admin);
            assert_eq!(rights.can_manage_users(), manage, "{rights:?}");
            assert_eq!(rights.can_manage_teams(), manage, "{rights:?}");
            assert_eq!(rights.can_export_backup(), export, "{rights:?}");
            assert_eq!(rights.shows_professional_offer(), offer, "{rights:?}");
        }
    }
}

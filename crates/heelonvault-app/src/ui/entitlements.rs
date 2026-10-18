// License entitlements for HeelonVault

pub struct Entitlements {
    pub can_manage_users: bool,
    pub shows_professional_offer: bool,
}

impl Entitlements {
    pub fn new() -> Self {
        Entitlements {
            can_manage_users: false,
            shows_professional_offer: false,
        }
    }

    pub fn can_manage_users(&self) -> bool {
        self.can_manage_users
    }

    pub fn shows_professional_offer(&self) -> bool {
        self.shows_professional_offer
    }
}

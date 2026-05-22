use crate::models::LicenseTier;

/// Abstraction over license state.  Always compiled — Core code can depend on
/// this trait without pulling in the `licensing` feature.
pub trait LicenseProvider: Send + Sync + 'static {
    fn tier(&self) -> LicenseTier;
    fn customer_name(&self) -> Option<String>;
    fn is_expired(&self) -> bool;
    fn has_user_capacity(&self, current_count: u32) -> bool;
}

/// Community (no-op) implementation: always Community tier, never expired,
/// unlimited capacity.  Used when the `licensing` feature is disabled.
pub struct CommunityLicenseProvider;

impl LicenseProvider for CommunityLicenseProvider {
    fn tier(&self) -> LicenseTier {
        LicenseTier::Community
    }

    fn customer_name(&self) -> Option<String> {
        None
    }

    fn is_expired(&self) -> bool {
        false
    }

    fn has_user_capacity(&self, _current_count: u32) -> bool {
        true
    }
}

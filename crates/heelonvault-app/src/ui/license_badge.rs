//! License state as shown in the UI (login screen, header bar, audit reports).
//!
//! Carried as data rather than as a pre-rendered label so the text can be translated at
//! display time and re-translated on a language change.

use heelonvault_core::i18n::{I18nArg, tr, tr_args};
#[cfg(feature = "premium")]
use heelonvault_core::models::{License, LicenseTier};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LicenseDisplay {
    /// Customer name of a Professional license; `None` for the Community edition.
    professional_customer: Option<String>,
}

impl LicenseDisplay {
    #[cfg(any(not(feature = "premium"), test))]
    pub fn community() -> Self {
        Self::default()
    }

    #[cfg(feature = "premium")]
    pub fn from_license(license: Option<&License>) -> Self {
        let professional_customer = license
            .filter(|license| license.tier == LicenseTier::Professional)
            .map(|license| license.customer_name.trim().to_string())
            .filter(|name| !name.is_empty());
        Self {
            professional_customer,
        }
    }

    /// Customer name of a Professional license, as printed on the seal and in reports.
    pub fn customer_name(&self) -> Option<String> {
        self.professional_customer
            .as_ref()
            .map(|name| name.to_uppercase())
    }

    /// Localized badge text: "Licence free" or "Licence pro - <customer>".
    pub fn badge_text(&self) -> String {
        match &self.professional_customer {
            Some(name) => tr_args("license-badge-pro", &[("customer", I18nArg::Str(name))]),
            None => tr("license-badge-free"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LicenseDisplay;

    #[test]
    fn community_has_no_customer_name() {
        assert_eq!(LicenseDisplay::community().customer_name(), None);
    }

    #[cfg(feature = "premium")]
    fn license(
        tier: heelonvault_core::models::LicenseTier,
        customer: &str,
    ) -> heelonvault_core::models::License {
        heelonvault_core::models::License {
            id: "test".to_string(),
            customer_name: customer.to_string(),
            slots_count: 1,
            expiration_date: "2099-01-01T00:00:00Z".to_string(),
            features: Vec::new(),
            tier,
        }
    }

    #[cfg(feature = "premium")]
    #[test]
    fn professional_customer_name_is_trimmed_and_uppercased() {
        use heelonvault_core::models::LicenseTier;

        let pro = license(LicenseTier::Professional, "  Clinique du Parc ");
        let display = LicenseDisplay::from_license(Some(&pro));
        assert_eq!(display.customer_name().as_deref(), Some("CLINIQUE DU PARC"));
    }

    #[cfg(feature = "premium")]
    #[test]
    fn community_tier_blank_customer_and_missing_license_are_community() {
        use heelonvault_core::models::LicenseTier;

        let community = license(LicenseTier::Community, "Acme");
        let blank = license(LicenseTier::Professional, "  ");
        for candidate in [Some(&community), Some(&blank), None] {
            assert_eq!(
                LicenseDisplay::from_license(candidate),
                LicenseDisplay::community()
            );
        }
    }
}

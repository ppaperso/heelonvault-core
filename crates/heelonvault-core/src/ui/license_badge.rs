/// Helper utilities for displaying license status in ui elements.
use crate::models::{License, LicenseTier};
use gtk4::prelude::*;

pub struct LicenseStatusWidget;

impl LicenseStatusWidget {
    /// Create a license status label for LoginDialog footer.
    /// Format: "[Community] Edition" or "[Professional] { name }"
    pub fn create_status_label(license: Option<&License>) -> gtk4::Label {
        let label = gtk4::Label::new(None);
        label.set_css_classes(&["dim-label"]); // Subtle styling

        if let Some(lic) = license {
            match lic.tier {
                LicenseTier::Community => {
                    label.set_text("Community Edition");
                    label.set_css_classes(&["dim-label"]);
                }
                LicenseTier::Professional => {
                    let text = format!("Pro: {}", lic.customer_name);
                    label.set_text(&text);
                    label.set_css_classes(&["success"]);
                }
            }
        } else {
            label.set_text("Community Edition");
            label.set_css_classes(&["dim-label"]);
        }

        label
    }

    /// Create a license status badge for MainWindow sidebar.
    /// Returns a box with badge styling.
    pub fn create_status_badge(license: Option<&License>) -> gtk4::Box {
        let badge_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        badge_box.set_margin_top(12);
        badge_box.set_margin_bottom(12);
        badge_box.set_margin_start(12);
        badge_box.set_margin_end(12);

        if let Some(lic) = license {
            match lic.tier {
                LicenseTier::Community => {
                    let label = gtk4::Label::new(Some("Community"));
                    label.set_css_classes(&["license-badge-community"]);
                    badge_box.append(&label);
                }
                LicenseTier::Professional => {
                    let label = gtk4::Label::new(Some(&format!("Pro: {}", lic.customer_name)));
                    label.set_css_classes(&["license-badge-professional"]);
                    badge_box.append(&label);
                }
            }
        } else {
            let label = gtk4::Label::new(Some("Community"));
            label.set_css_classes(&["license-badge-community"]);
            badge_box.append(&label);
        }

        badge_box
    }
}

use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Orientation};

use crate::ui::license_badge::LicenseDisplay;

/// Header license badge (Community) or seal (Professional), plus the callback that
/// re-applies its translation on a language change.
pub(super) fn build_header_license_badge(license: &LicenseDisplay) -> (gtk4::Widget, Rc<dyn Fn()>) {
    if let Some(customer_name) = license.customer_name() {
        let seal = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .valign(Align::Center)
            .build();
        seal.add_css_class("header-badge");
        seal.add_css_class("heelonys-seal");
        seal.add_css_class("heelonys-seal-compact");

        let shield_icon = gtk4::Image::from_icon_name("security-high-symbolic");
        shield_icon.add_css_class("heelonys-seal-shield");

        let cert_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("license-seal-certified").as_str(),
        ));
        cert_label.add_css_class("heelonys-seal-cert");

        let divider = gtk4::Separator::new(Orientation::Vertical);
        divider.add_css_class("heelonys-seal-divider");

        let customer_label = gtk4::Label::new(Some(customer_name.as_str()));
        customer_label.add_css_class("heelonys-seal-customer");

        seal.append(&shield_icon);
        seal.append(&cert_label);
        seal.append(&divider);
        seal.append(&customer_label);

        let refresh: Rc<dyn Fn()> = Rc::new(move || {
            cert_label.set_text(heelonvault_core::tr!("license-seal-certified").as_str());
        });
        (seal.upcast::<gtk4::Widget>(), refresh)
    } else {
        let badge = gtk4::Label::new(Some(license.badge_text().as_str()));
        badge.add_css_class("header-badge");
        badge.add_css_class("license-badge-community");
        let license = license.clone();
        let badge_for_refresh = badge.clone();
        let refresh: Rc<dyn Fn()> = Rc::new(move || {
            badge_for_refresh.set_text(license.badge_text().as_str());
        });
        (badge.upcast::<gtk4::Widget>(), refresh)
    }
}

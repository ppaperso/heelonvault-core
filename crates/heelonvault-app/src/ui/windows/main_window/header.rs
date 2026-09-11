use gtk4::prelude::*;
use gtk4::{Align, Orientation};

use super::MainWindow;

pub(super) fn build_header_license_badge(license_badge_text: &str) -> gtk4::Widget {
    if let Some(customer_name) = MainWindow::professional_customer_name(license_badge_text) {
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

        let cert_label = gtk4::Label::new(Some("HEELONYS CERTIFIED"));
        cert_label.add_css_class("heelonys-seal-cert");

        let divider = gtk4::Separator::new(Orientation::Vertical);
        divider.add_css_class("heelonys-seal-divider");

        let customer_label = gtk4::Label::new(Some(customer_name.as_str()));
        customer_label.add_css_class("heelonys-seal-customer");

        seal.append(&shield_icon);
        seal.append(&cert_label);
        seal.append(&divider);
        seal.append(&customer_label);

        seal.upcast::<gtk4::Widget>()
    } else {
        let badge = gtk4::Label::new(Some(license_badge_text));
        badge.add_css_class("header-badge");
        badge.add_css_class("license-badge-community");
        badge.upcast::<gtk4::Widget>()
    }
}

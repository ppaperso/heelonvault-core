//! Master password change and master key rotation.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone)]
pub struct PasswordSection {
    pub frame: adw::PreferencesGroup,
    pub subtitle: gtk4::Label,
    pub current_entry: adw::PasswordEntryRow,
    pub new_entry: adw::PasswordEntryRow,
    pub confirm_entry: adw::PasswordEntryRow,
    pub status_label: gtk4::Label,
    pub actions_row: gtk4::Box,
    pub change_button: gtk4::Button,
    pub rotate_button: gtk4::Button,
}

pub fn build() -> PasswordSection {
    let password_change_frame = adw::PreferencesGroup::new();
    password_change_frame
        .set_title(heelonvault_core::tr!("profile-section-password-change").as_str());
    password_change_frame.add_css_class("profile-section-frame");
    let password_change_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let password_change_subtitle = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-section-password-change-subtitle").as_str(),
    ));
    password_change_subtitle.set_halign(Align::Start);
    password_change_subtitle.set_wrap(true);
    password_change_subtitle.add_css_class("profile-section-subtitle");
    password_change_subtitle.add_css_class("dim-label");
    password_change_box.append(&password_change_subtitle);

    let current_pw_entry = adw::PasswordEntryRow::new();
    current_pw_entry.set_title(heelonvault_core::tr!("profile-field-current-password").as_str());
    current_pw_entry.set_hexpand(true);
    current_pw_entry.add_css_class("profile-field-entry");

    let new_pw_entry = adw::PasswordEntryRow::new();
    new_pw_entry.set_title(heelonvault_core::tr!("profile-field-new-password").as_str());
    new_pw_entry.set_hexpand(true);
    new_pw_entry.add_css_class("profile-field-entry");

    let confirm_pw_entry = adw::PasswordEntryRow::new();
    confirm_pw_entry
        .set_title(heelonvault_core::tr!("profile-field-confirm-new-password").as_str());
    confirm_pw_entry.set_hexpand(true);
    confirm_pw_entry.add_css_class("profile-field-entry");

    let password_change_status_label = gtk4::Label::new(None);
    password_change_status_label.set_halign(Align::Start);
    password_change_status_label.set_wrap(true);
    password_change_status_label.add_css_class("inline-status");
    password_change_status_label.add_css_class("profile-inline-status");
    password_change_status_label.set_visible(false);

    let security_actions = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .hexpand(true)
        .halign(Align::End)
        .build();
    security_actions.add_css_class("profile-actions-row");
    let change_pw_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-change-button").as_str());
    change_pw_button.add_css_class("suggested-action");
    change_pw_button.add_css_class("profile-action-btn");
    change_pw_button.set_tooltip_text(Some(
        heelonvault_core::tr!("profile-change-button-tooltip").as_str(),
    ));
    let rotate_master_key_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-rotate-button").as_str());
    rotate_master_key_button.add_css_class("suggested-action");
    rotate_master_key_button.add_css_class("profile-action-btn");
    rotate_master_key_button.set_tooltip_text(Some(
        heelonvault_core::tr!("profile-rotate-button-tooltip").as_str(),
    ));
    security_actions.append(&change_pw_button);
    security_actions.append(&rotate_master_key_button);

    for widget in [
        current_pw_entry.upcast_ref::<gtk4::Widget>(),
        new_pw_entry.upcast_ref::<gtk4::Widget>(),
        confirm_pw_entry.upcast_ref::<gtk4::Widget>(),
        password_change_status_label.upcast_ref::<gtk4::Widget>(),
        security_actions.upcast_ref::<gtk4::Widget>(),
    ] {
        password_change_box.append(widget);
    }
    password_change_frame.add(&password_change_box);
    password_change_frame.set_hexpand(true);
    PasswordSection {
        frame: password_change_frame,
        subtitle: password_change_subtitle,
        current_entry: current_pw_entry,
        new_entry: new_pw_entry,
        confirm_entry: confirm_pw_entry,
        status_label: password_change_status_label,
        actions_row: security_actions,
        change_button: change_pw_button,
        rotate_button: rotate_master_key_button,
    }
}

impl PasswordSection {
    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-password-change").as_str());
        self.subtitle
            .set_text(heelonvault_core::tr!("profile-section-password-change-subtitle").as_str());
        self.current_entry
            .set_title(heelonvault_core::tr!("profile-field-current-password").as_str());
        self.new_entry
            .set_title(heelonvault_core::tr!("profile-field-new-password").as_str());
        self.confirm_entry
            .set_title(heelonvault_core::tr!("profile-field-confirm-new-password").as_str());
        self.change_button
            .set_label(heelonvault_core::tr!("profile-change-button").as_str());
        self.change_button.set_tooltip_text(Some(
            heelonvault_core::tr!("profile-change-button-tooltip").as_str(),
        ));
        self.rotate_button
            .set_label(heelonvault_core::tr!("profile-rotate-button").as_str());
        self.rotate_button.set_tooltip_text(Some(
            heelonvault_core::tr!("profile-rotate-button-tooltip").as_str(),
        ));
    }
}

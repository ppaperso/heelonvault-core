//! Master password change, as an expandable row of the "Sign-in and protection" group: the
//! fields only appear when the user asks for them, with the policy and a strength gauge.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::ui::widgets::password_strength_bar::PasswordStrengthBar;

/// Same minimum as `UserService::change_master_password`, counted in characters here (a
/// character is at least one byte, so this never accepts what the service would refuse).
pub const MIN_MASTER_PASSWORD_CHARS: usize = 16;

#[derive(Clone)]
pub struct PasswordSection {
    pub expander: adw::ExpanderRow,
    pub current_entry: adw::PasswordEntryRow,
    pub new_entry: adw::PasswordEntryRow,
    pub confirm_entry: adw::PasswordEntryRow,
    pub status_label: gtk4::Label,
    pub rotate_button: gtk4::Button,
    policy: gtk4::Label,
}

pub fn build() -> PasswordSection {
    let expander = adw::ExpanderRow::new();
    let icon = gtk4::Image::from_icon_name("hv-key-symbolic");
    icon.add_css_class("profile-row-icon");
    expander.add_prefix(&icon);

    let current_entry = adw::PasswordEntryRow::new();
    let new_entry = adw::PasswordEntryRow::new();
    let confirm_entry = adw::PasswordEntryRow::new();
    // Stored as typed: the login screen does not trim either.
    let strength = PasswordStrengthBar::for_stored_passwords();
    strength.connect_to_password_entry_row(&new_entry);
    expander.add_row(&current_entry);
    expander.add_row(&new_entry);
    expander.add_row(&strength.as_action_row());
    expander.add_row(&confirm_entry);

    let footer = gtk4::Box::new(Orientation::Vertical, 10);
    footer.add_css_class("profile-row-footer");
    let policy = gtk4::Label::new(None);
    policy.set_xalign(0.0);
    policy.set_wrap(true);
    policy.add_css_class("dim-label");
    policy.add_css_class("caption");
    let status_label = gtk4::Label::new(None);
    status_label.set_halign(Align::Start);
    status_label.set_wrap(true);
    status_label.add_css_class("inline-status");
    status_label.add_css_class("profile-inline-status");
    status_label.set_visible(false);
    let rotate_button = gtk4::Button::new();
    rotate_button.add_css_class("suggested-action");
    rotate_button.set_halign(Align::End);
    footer.append(&policy);
    footer.append(&status_label);
    footer.append(&rotate_button);
    let footer_row = gtk4::ListBoxRow::new();
    footer_row.set_activatable(false);
    footer_row.set_child(Some(&footer));
    expander.add_row(&footer_row);

    let section = PasswordSection {
        expander,
        current_entry,
        new_entry,
        confirm_entry,
        status_label,
        rotate_button,
        policy,
    };
    section.refresh_i18n();
    section
}

impl PasswordSection {
    pub fn refresh_i18n(&self) {
        self.expander
            .set_title(heelonvault_core::tr!("profile-section-password-change").as_str());
        self.expander.set_subtitle(
            heelonvault_core::tr!("profile-section-password-change-subtitle").as_str(),
        );
        self.current_entry
            .set_title(heelonvault_core::tr!("profile-field-current-password").as_str());
        self.new_entry
            .set_title(heelonvault_core::tr!("profile-field-new-password").as_str());
        self.confirm_entry
            .set_title(heelonvault_core::tr!("profile-field-confirm-new-password").as_str());
        self.policy.set_text(
            heelonvault_core::i18n::tr_args(
                "profile-password-policy",
                &[(
                    "min",
                    heelonvault_core::i18n::I18nArg::Num(MIN_MASTER_PASSWORD_CHARS as i64),
                )],
            )
            .as_str(),
        );
        self.rotate_button
            .set_label(heelonvault_core::tr!("profile-rotate-button").as_str());
        self.rotate_button.set_tooltip_text(Some(
            heelonvault_core::tr!("profile-rotate-button-tooltip").as_str(),
        ));
    }
}

//! Account identity: username, display name, email and language.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone)]
pub struct IdentitySection {
    pub frame: adw::PreferencesGroup,
    pub subtitle: gtk4::Label,
    pub username_entry: adw::EntryRow,
    pub display_entry: adw::EntryRow,
    pub email_entry: adw::EntryRow,
    pub language_label: gtk4::Label,
    pub language_hint: gtk4::Label,
    pub language_items: gtk4::StringList,
    pub language_dropdown: gtk4::DropDown,
    pub current_email_pw_entry: adw::PasswordEntryRow,
    pub status_label: gtk4::Label,
    pub save_row: gtk4::Box,
    pub save_button: gtk4::Button,
}

pub fn build() -> IdentitySection {
    let info_frame = adw::PreferencesGroup::new();
    info_frame.set_title(heelonvault_core::tr!("profile-section-info").as_str());
    info_frame.add_css_class("profile-section-frame");
    let info_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let info_subtitle = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-section-info-subtitle").as_str(),
    ));
    info_subtitle.set_halign(Align::Start);
    info_subtitle.set_wrap(true);
    info_subtitle.add_css_class("profile-section-subtitle");
    info_subtitle.add_css_class("dim-label");
    info_box.append(&info_subtitle);
    let username_entry = adw::EntryRow::new();
    username_entry.set_title(heelonvault_core::tr!("profile-field-username").as_str());
    username_entry.set_sensitive(false);
    username_entry.set_hexpand(true);
    username_entry.add_css_class("profile-field-entry");
    let display_entry = adw::EntryRow::new();
    display_entry.set_title(heelonvault_core::tr!("profile-field-display-name").as_str());
    display_entry.set_hexpand(true);
    display_entry.add_css_class("profile-field-entry");
    let email_entry = adw::EntryRow::new();
    email_entry.set_title(heelonvault_core::tr!("profile-field-email").as_str());
    email_entry.set_hexpand(true);
    email_entry.add_css_class("profile-field-entry");
    let language_label = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-language-title").as_str(),
    ));
    language_label.set_halign(Align::Start);
    language_label.add_css_class("profile-field-label");
    let language_hint = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-language-subtitle").as_str(),
    ));
    language_hint.set_halign(Align::Start);
    language_hint.set_wrap(true);
    language_hint.add_css_class("dim-label");
    language_hint.add_css_class("profile-section-subtitle");
    let language_items = gtk4::StringList::new(&[
        heelonvault_core::tr!("language-option-fr").as_str(),
        heelonvault_core::tr!("language-option-en").as_str(),
    ]);
    let language_dropdown =
        gtk4::DropDown::new(Some(language_items.clone()), None::<gtk4::Expression>);
    language_dropdown.add_css_class("profile-field-entry");
    language_dropdown.set_selected(0);
    let current_email_pw_entry = adw::PasswordEntryRow::new();
    current_email_pw_entry
        .set_title(heelonvault_core::tr!("profile-field-current-password-email-change").as_str());
    current_email_pw_entry.set_hexpand(true);
    current_email_pw_entry.add_css_class("profile-field-entry");
    let profile_status_label = gtk4::Label::new(None);
    profile_status_label.set_halign(Align::Start);
    profile_status_label.set_wrap(true);
    profile_status_label.add_css_class("inline-status");
    profile_status_label.add_css_class("profile-inline-status");
    profile_status_label.set_visible(false);
    let save_profile_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .hexpand(true)
        .halign(Align::End)
        .build();
    save_profile_row.add_css_class("profile-actions-row");
    let save_profile_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-save").as_str());
    save_profile_button.add_css_class("suggested-action");
    save_profile_button.add_css_class("profile-action-btn");
    save_profile_row.append(&save_profile_button);

    for widget in [
        username_entry.upcast_ref::<gtk4::Widget>(),
        display_entry.upcast_ref::<gtk4::Widget>(),
        email_entry.upcast_ref::<gtk4::Widget>(),
        language_label.upcast_ref::<gtk4::Widget>(),
        language_dropdown.upcast_ref::<gtk4::Widget>(),
        language_hint.upcast_ref::<gtk4::Widget>(),
        current_email_pw_entry.upcast_ref::<gtk4::Widget>(),
        profile_status_label.upcast_ref::<gtk4::Widget>(),
        save_profile_row.upcast_ref::<gtk4::Widget>(),
    ] {
        info_box.append(widget);
    }
    info_frame.add(&info_box);
    info_frame.set_hexpand(true);
    IdentitySection {
        frame: info_frame,
        subtitle: info_subtitle,
        username_entry,
        display_entry,
        email_entry,
        language_label,
        language_hint,
        language_items,
        language_dropdown,
        current_email_pw_entry,
        status_label: profile_status_label,
        save_row: save_profile_row,
        save_button: save_profile_button,
    }
}

impl IdentitySection {
    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-info").as_str());
        self.subtitle
            .set_text(heelonvault_core::tr!("profile-section-info-subtitle").as_str());
        self.username_entry
            .set_title(heelonvault_core::tr!("profile-field-username").as_str());
        self.display_entry
            .set_title(heelonvault_core::tr!("profile-field-display-name").as_str());
        self.email_entry
            .set_title(heelonvault_core::tr!("profile-field-email").as_str());
        self.language_label
            .set_text(heelonvault_core::tr!("profile-language-title").as_str());
        self.language_hint
            .set_text(heelonvault_core::tr!("profile-language-subtitle").as_str());
        self.current_email_pw_entry.set_title(
            heelonvault_core::tr!("profile-field-current-password-email-change").as_str(),
        );
        self.save_button
            .set_label(heelonvault_core::tr!("profile-save").as_str());

        // Re-labelling the model resets the selection, so restore it afterwards.
        let selected = self.language_dropdown.selected();
        self.language_items.splice(
            0,
            self.language_items.n_items(),
            &[
                heelonvault_core::tr!("language-option-fr").as_str(),
                heelonvault_core::tr!("language-option-en").as_str(),
            ],
        );
        self.language_dropdown.set_selected(selected.min(1));
    }
}

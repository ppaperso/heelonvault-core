//! Account details: display name and e-mail. The username is read-only, and the current
//! password is only asked for when the e-mail actually changes.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;

/// Values last loaded from or saved to the database; the form compares against them.
#[derive(Clone, Debug, Default)]
pub struct SavedIdentity {
    pub display_name: String,
    pub email: String,
}

#[derive(Clone)]
pub struct IdentitySection {
    pub frame: adw::PreferencesGroup,
    pub username_entry: adw::EntryRow,
    pub display_entry: adw::EntryRow,
    pub email_entry: adw::EntryRow,
    pub current_email_pw_entry: adw::PasswordEntryRow,
    pub status_label: gtk4::Label,
    pub save_button: gtk4::Button,
    pub saved: Rc<RefCell<SavedIdentity>>,
}

fn entry_row(key: &str) -> adw::EntryRow {
    let row = adw::EntryRow::new();
    row.set_title(heelonvault_core::i18n::tr(key).as_str());
    row
}

pub fn build() -> IdentitySection {
    let frame = adw::PreferencesGroup::new();
    frame.add_css_class("profile-group");

    let username_entry = entry_row("profile-field-username");
    // Read-only but not greyed out: it is information, not a disabled control.
    username_entry.set_editable(false);
    username_entry.set_can_focus(false);
    username_entry.add_css_class("profile-readonly-row");
    let display_entry = entry_row("profile-field-display-name");
    let email_entry = entry_row("profile-field-email");
    let current_email_pw_entry = adw::PasswordEntryRow::new();
    current_email_pw_entry.set_visible(false);

    let save_button = gtk4::Button::new();
    save_button.add_css_class("profile-save-button");
    save_button.set_valign(Align::Center);
    save_button.set_sensitive(false);
    frame.set_header_suffix(Some(&save_button));

    for row in [&username_entry, &display_entry, &email_entry] {
        frame.add(row);
    }
    frame.add(&current_email_pw_entry);

    let status_label = gtk4::Label::new(None);
    status_label.set_halign(Align::Start);
    status_label.set_wrap(true);
    status_label.add_css_class("inline-status");
    status_label.add_css_class("profile-inline-status");
    status_label.set_visible(false);
    let status_box = gtk4::Box::new(Orientation::Vertical, 0);
    status_box.append(&status_label);
    frame.add(&status_box);

    let section = IdentitySection {
        frame,
        username_entry,
        display_entry,
        email_entry,
        current_email_pw_entry,
        status_label,
        save_button,
        saved: Rc::new(RefCell::new(SavedIdentity::default())),
    };
    section.refresh_i18n();

    let dirty_check = {
        let section = section.clone();
        move || section.update_dirty_state()
    };
    for row in [&section.display_entry, &section.email_entry] {
        let check = dirty_check.clone();
        row.connect_changed(move |_| check());
    }
    section
}

impl IdentitySection {
    pub fn email_changed(&self) -> bool {
        self.email_entry.text().trim() != self.saved.borrow().email
    }

    /// Records what is now stored and resets the form state.
    pub fn mark_saved(&self, display_name: &str, email: &str) {
        *self.saved.borrow_mut() = SavedIdentity {
            display_name: display_name.to_string(),
            email: email.to_string(),
        };
        self.current_email_pw_entry.set_text("");
        self.update_dirty_state();
    }

    fn update_dirty_state(&self) {
        let dirty = {
            let saved = self.saved.borrow();
            self.display_entry.text().trim() != saved.display_name
                || self.email_entry.text().trim() != saved.email
        };
        self.current_email_pw_entry
            .set_visible(self.email_changed());
        self.save_button.set_sensitive(dirty);
        // The only primary action of the group, highlighted only when there is something to save.
        if dirty {
            self.save_button.add_css_class("suggested-action");
        } else {
            self.save_button.remove_css_class("suggested-action");
        }
    }

    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-info").as_str());
        self.frame.set_description(Some(
            heelonvault_core::tr!("profile-section-info-subtitle").as_str(),
        ));
        self.username_entry
            .set_title(heelonvault_core::tr!("profile-field-username").as_str());
        self.display_entry
            .set_title(heelonvault_core::tr!("profile-field-display-name").as_str());
        self.email_entry
            .set_title(heelonvault_core::tr!("profile-field-email").as_str());
        self.current_email_pw_entry.set_title(
            heelonvault_core::tr!("profile-field-current-password-email-change").as_str(),
        );
        self.save_button
            .set_label(heelonvault_core::tr!("profile-save").as_str());
    }
}

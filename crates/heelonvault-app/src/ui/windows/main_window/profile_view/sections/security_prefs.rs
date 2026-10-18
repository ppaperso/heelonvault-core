//! Session settings (auto-lock delay, password visibility in the editor) and the PIN
//! quick-unlock row, which the page shows with the other sign-in protections.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;

/// Auto-lock choices, in display order; the last one disables auto-lock.
pub const AUTO_LOCK_MINUTES: [i64; 4] = [1, 5, 30, 0];
const AUTO_LOCK_KEYS: [&str; 4] = [
    "profile-auto-lock-1",
    "profile-auto-lock-5",
    "profile-auto-lock-30",
    "profile-auto-lock-never",
];

#[derive(Clone)]
pub struct SecurityPrefsSection {
    pub frame: adw::PreferencesGroup,
    pub auto_lock_items: gtk4::StringList,
    pub auto_lock_dropdown: adw::ComboRow,
    pub show_passwords_switch: adw::SwitchRow,
    pub pin_row: adw::ActionRow,
    pub pin_status_badge: gtk4::Label,
    pub pin_setup_button: gtk4::Button,
    pub status_label: gtk4::Label,
}

fn translated_auto_lock_items() -> Vec<String> {
    AUTO_LOCK_KEYS
        .iter()
        .map(|key| heelonvault_core::i18n::tr(key))
        .collect()
}

pub fn build(pin_active: bool) -> SecurityPrefsSection {
    let frame = adw::PreferencesGroup::new();
    frame.add_css_class("profile-group");

    let labels = translated_auto_lock_items();
    let auto_lock_items =
        gtk4::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    let auto_lock_dropdown = adw::ComboRow::new();
    auto_lock_dropdown.set_model(Some(&auto_lock_items));
    let lock_icon = gtk4::Image::from_icon_name("hv-lock-symbolic");
    lock_icon.add_css_class("profile-row-icon");
    auto_lock_dropdown.add_prefix(&lock_icon);
    let show_passwords_switch = adw::SwitchRow::new();
    frame.add(&auto_lock_dropdown);
    frame.add(&show_passwords_switch);

    let status_label = gtk4::Label::new(None);
    status_label.set_halign(Align::Start);
    status_label.set_wrap(true);
    status_label.add_css_class("inline-status");
    status_label.add_css_class("profile-inline-status");
    status_label.set_visible(false);
    let status_box = gtk4::Box::new(Orientation::Vertical, 0);
    status_box.append(&status_label);
    frame.add(&status_box);

    let pin_row = adw::ActionRow::new();
    let pin_icon = gtk4::Image::from_icon_name("hv-lock-symbolic");
    pin_icon.add_css_class("profile-row-icon");
    pin_row.add_prefix(&pin_icon);
    let pin_status_badge = gtk4::Label::new(None);
    pin_status_badge.add_css_class("status-role-pill");
    pin_status_badge.set_valign(Align::Center);
    pin_status_badge.add_css_class(if pin_active {
        "status-role-user"
    } else {
        "status-role-disabled"
    });
    let pin_setup_button = gtk4::Button::new();
    pin_setup_button.add_css_class("flat");
    pin_setup_button.add_css_class("profile-row-button");
    pin_setup_button.set_valign(Align::Center);
    pin_row.add_suffix(&pin_status_badge);
    pin_row.add_suffix(&pin_setup_button);
    pin_row.set_activatable_widget(Some(&pin_setup_button));

    let section = SecurityPrefsSection {
        frame,
        auto_lock_items,
        auto_lock_dropdown,
        show_passwords_switch,
        pin_row,
        pin_status_badge,
        pin_setup_button,
        status_label,
    };
    section.set_pin_active(pin_active);
    section.refresh_i18n();
    section
}

impl SecurityPrefsSection {
    pub fn set_pin_active(&self, active: bool) {
        self.pin_status_badge.set_text(
            heelonvault_core::tr!(if active {
                "pin-status-active"
            } else {
                "pin-status-inactive"
            })
            .as_str(),
        );
        let (add, remove) = if active {
            ("status-role-user", "status-role-disabled")
        } else {
            ("status-role-disabled", "status-role-user")
        };
        self.pin_status_badge.remove_css_class(remove);
        self.pin_status_badge.add_css_class(add);
    }

    pub fn auto_lock_enabled(&self) -> bool {
        AUTO_LOCK_MINUTES
            .get(self.auto_lock_dropdown.selected() as usize)
            .is_some_and(|minutes| *minutes != 0)
    }

    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-security-prefs").as_str());
        self.auto_lock_dropdown
            .set_title(heelonvault_core::tr!("profile-auto-lock-title").as_str());
        self.auto_lock_dropdown
            .set_subtitle(heelonvault_core::tr!("profile-auto-lock-subtitle").as_str());
        self.show_passwords_switch
            .set_title(heelonvault_core::tr!("profile-show-edit-passwords-title").as_str());
        self.show_passwords_switch
            .set_subtitle(heelonvault_core::tr!("profile-show-edit-passwords-hint").as_str());
        self.pin_row
            .set_title(heelonvault_core::tr!("profile-pin-title").as_str());
        self.pin_row
            .set_subtitle(heelonvault_core::tr!("profile-pin-hint").as_str());
        self.pin_setup_button
            .set_label(heelonvault_core::tr!("profile-pin-configure").as_str());

        let selected = self.auto_lock_dropdown.selected();
        let labels = translated_auto_lock_items();
        self.auto_lock_items.splice(
            0,
            self.auto_lock_items.n_items(),
            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        self.auto_lock_dropdown.set_selected(selected);
    }
}

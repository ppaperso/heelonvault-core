//! Auto-lock delay, password visibility and PIN quick-unlock.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone)]
pub struct SecurityPrefsSection {
    pub frame: adw::PreferencesGroup,
    pub auto_lock_label: gtk4::Label,
    pub auto_lock_items: gtk4::StringList,
    pub auto_lock_dropdown: gtk4::DropDown,
    pub show_passwords_switch: adw::SwitchRow,
    pub show_passwords_hint: gtk4::Label,
    pub pin_status_badge: gtk4::Label,
    pub pin_setup_button: gtk4::Button,
    pub status_label: gtk4::Label,
}

pub fn build(pin_active: bool) -> SecurityPrefsSection {
    let security_prefs_frame = adw::PreferencesGroup::new();
    security_prefs_frame
        .set_title(heelonvault_core::tr!("profile-section-security-prefs").as_str());
    security_prefs_frame.add_css_class("profile-section-frame");
    let security_prefs_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let auto_lock_label = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-auto-lock-title").as_str(),
    ));
    auto_lock_label.set_halign(Align::Start);
    auto_lock_label.add_css_class("profile-field-label");
    let auto_lock_items = gtk4::StringList::new(&[
        heelonvault_core::tr!("profile-auto-lock-1").as_str(),
        heelonvault_core::tr!("profile-auto-lock-5").as_str(),
        heelonvault_core::tr!("profile-auto-lock-30").as_str(),
        heelonvault_core::tr!("profile-auto-lock-never").as_str(),
    ]);
    let auto_lock_dropdown =
        gtk4::DropDown::new(Some(auto_lock_items.clone()), None::<gtk4::Expression>);
    auto_lock_dropdown.add_css_class("profile-field-entry");

    let show_edit_passwords_switch = adw::SwitchRow::new();
    show_edit_passwords_switch
        .set_title(heelonvault_core::tr!("profile-show-edit-passwords-title").as_str());
    show_edit_passwords_switch.add_css_class("profile-field-entry");
    let show_edit_passwords_hint = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-show-edit-passwords-hint").as_str(),
    ));
    show_edit_passwords_hint.set_halign(Align::Start);
    show_edit_passwords_hint.set_wrap(true);
    show_edit_passwords_hint.add_css_class("dim-label");
    show_edit_passwords_hint.add_css_class("profile-section-subtitle");

    // ── PIN quick-unlock section ─────────────────────────────────────────
    let pin_separator = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    pin_separator.add_css_class("profile-section-separator");

    let pin_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(12)
        .hexpand(true)
        .build();
    let pin_label_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    let pin_title = gtk4::Label::new(Some(heelonvault_core::tr!("profile-pin-title").as_str()));
    pin_title.set_halign(Align::Start);
    pin_title.add_css_class("profile-field-label");
    let pin_hint = gtk4::Label::new(Some(heelonvault_core::tr!("profile-pin-hint").as_str()));
    pin_hint.set_halign(Align::Start);
    pin_hint.set_wrap(true);
    pin_hint.add_css_class("dim-label");
    pin_hint.add_css_class("caption");
    pin_label_box.append(&pin_title);
    pin_label_box.append(&pin_hint);
    let pin_status_badge = gtk4::Label::new(None);
    pin_status_badge.add_css_class("status-role-pill");
    pin_status_badge.set_valign(Align::Center);
    if pin_active {
        pin_status_badge.set_text(heelonvault_core::tr!("pin-status-active").as_str());
        pin_status_badge.add_css_class("status-role-user");
    } else {
        pin_status_badge.set_text(heelonvault_core::tr!("pin-status-inactive").as_str());
        pin_status_badge.add_css_class("status-role-disabled");
    }
    pin_row.append(&pin_label_box);
    pin_row.append(&pin_status_badge);
    let pin_setup_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-pin-configure").as_str());
    pin_setup_button.add_css_class("profile-action-btn");
    pin_setup_button.add_css_class("flat");
    pin_row.append(&pin_setup_button);

    let security_prefs_status_label = gtk4::Label::new(None);
    security_prefs_status_label.set_halign(Align::Start);
    security_prefs_status_label.set_wrap(true);
    security_prefs_status_label.add_css_class("inline-status");
    security_prefs_status_label.add_css_class("profile-inline-status");
    security_prefs_status_label.set_visible(false);

    for widget in [
        auto_lock_label.upcast_ref::<gtk4::Widget>(),
        auto_lock_dropdown.upcast_ref::<gtk4::Widget>(),
        show_edit_passwords_switch.upcast_ref::<gtk4::Widget>(),
        show_edit_passwords_hint.upcast_ref::<gtk4::Widget>(),
        pin_separator.upcast_ref::<gtk4::Widget>(),
        pin_row.upcast_ref::<gtk4::Widget>(),
        security_prefs_status_label.upcast_ref::<gtk4::Widget>(),
    ] {
        security_prefs_box.append(widget);
    }
    security_prefs_frame.add(&security_prefs_box);
    security_prefs_frame.set_hexpand(true);

    // 2FA section: AdwPreferencesGroup + collapsed ExpanderRow (progressive disclosure)
    SecurityPrefsSection {
        frame: security_prefs_frame,
        auto_lock_label,
        auto_lock_items,
        auto_lock_dropdown,
        show_passwords_switch: show_edit_passwords_switch,
        show_passwords_hint: show_edit_passwords_hint,
        pin_status_badge,
        pin_setup_button,
        status_label: security_prefs_status_label,
    }
}

impl SecurityPrefsSection {
    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-security-prefs").as_str());
        self.auto_lock_label
            .set_text(heelonvault_core::tr!("profile-auto-lock-title").as_str());
        self.show_passwords_switch
            .set_title(heelonvault_core::tr!("profile-show-edit-passwords-title").as_str());
        self.show_passwords_hint
            .set_text(heelonvault_core::tr!("profile-show-edit-passwords-hint").as_str());

        // Re-labelling the model resets the selection, so restore it afterwards.
        let selected = self.auto_lock_dropdown.selected();
        self.auto_lock_items.splice(
            0,
            self.auto_lock_items.n_items(),
            &[
                heelonvault_core::tr!("profile-auto-lock-1").as_str(),
                heelonvault_core::tr!("profile-auto-lock-5").as_str(),
                heelonvault_core::tr!("profile-auto-lock-30").as_str(),
                heelonvault_core::tr!("profile-auto-lock-never").as_str(),
            ],
        );
        self.auto_lock_dropdown.set_selected(selected.min(3));
    }
}

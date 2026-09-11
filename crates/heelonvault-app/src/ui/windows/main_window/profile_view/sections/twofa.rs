//! Two-factor authentication (TOTP) section.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::ui::messages;
use crate::ui::windows::main_window::MainWindow;

#[derive(Clone)]
pub struct TwoFaSection {
    pub group: adw::PreferencesGroup,
    pub expander: adw::ExpanderRow,
    pub state_badge: gtk4::Label,
    pub stack: gtk4::Stack,
    pub disabled_box: gtk4::Box,
    pub disabled_copy: gtk4::Label,
    pub activate_button: gtk4::Button,
    pub setup_box: gtk4::Box,
    pub setup_copy: gtk4::Label,
    pub qr_picture: gtk4::Picture,
    pub secret_label: gtk4::Label,
    pub secret_entry: gtk4::Entry,
    pub code_label: gtk4::Label,
    pub code_entry: gtk4::Entry,
    pub confirm_button: gtk4::Button,
    pub cancel_setup_button: gtk4::Button,
    pub enabled_box: gtk4::Box,
    pub enabled_copy: gtk4::Label,
    pub disable_toggle_button: gtk4::Button,
    pub disable_confirm_row: gtk4::Box,
    pub disable_confirm_button: gtk4::Button,
    pub disable_cancel_button: gtk4::Button,
    pub status_label: gtk4::Label,
}

pub fn build() -> TwoFaSection {
    let twofa_group = adw::PreferencesGroup::new();
    twofa_group.set_hexpand(true);
    let twofa_expander = adw::ExpanderRow::new();
    twofa_expander.set_title(heelonvault_core::tr!("profile-section-2fa-title").as_str());
    twofa_expander.set_expanded(false);
    let twofa_badge_text = messages::twofa_badge_disabled();
    let twofa_state_badge = gtk4::Label::new(Some(twofa_badge_text.as_str()));
    twofa_state_badge.add_css_class("status-role-pill");
    twofa_state_badge.add_css_class("status-role-user");
    twofa_state_badge.set_valign(Align::Center);
    twofa_expander.add_suffix(&twofa_state_badge);
    MainWindow::set_twofa_badge_state(&twofa_state_badge, false);
    let twofa_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let twofa_stack = gtk4::Stack::builder()
        .hexpand(true)
        .transition_type(gtk4::StackTransitionType::Crossfade)
        .build();

    let twofa_disabled_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    let twofa_disabled_copy = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-2fa-disabled-copy").as_str(),
    ));
    twofa_disabled_copy.set_halign(Align::Start);
    twofa_disabled_copy.set_wrap(true);
    twofa_disabled_copy.add_css_class("dim-label");
    twofa_disabled_copy.add_css_class("profile-section-subtitle");
    let twofa_activate_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-2fa-activate").as_str());
    twofa_activate_button.add_css_class("suggested-action");
    twofa_activate_button.add_css_class("profile-action-btn");
    twofa_activate_button.set_halign(Align::Start);
    twofa_disabled_box.append(&twofa_disabled_copy);
    twofa_disabled_box.append(&twofa_activate_button);

    let twofa_setup_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    let twofa_setup_copy = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-2fa-setup-copy").as_str(),
    ));
    twofa_setup_copy.set_halign(Align::Start);
    twofa_setup_copy.set_wrap(true);
    twofa_setup_copy.add_css_class("dim-label");
    twofa_setup_copy.add_css_class("profile-section-subtitle");
    let twofa_qr_picture = gtk4::Picture::new();
    twofa_qr_picture.set_size_request(200, 200);
    twofa_qr_picture.add_css_class("totp-qr-container");
    let twofa_secret_label = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-2fa-secret-base32").as_str(),
    ));
    twofa_secret_label.set_halign(Align::Start);
    twofa_secret_label.add_css_class("profile-field-label");
    let twofa_secret_entry = gtk4::Entry::new();
    twofa_secret_entry.set_editable(false);
    twofa_secret_entry.add_css_class("profile-field-entry");
    let twofa_code_label =
        gtk4::Label::new(Some(heelonvault_core::tr!("profile-2fa-code").as_str()));
    twofa_code_label.set_halign(Align::Start);
    twofa_code_label.add_css_class("profile-field-label");
    let twofa_code_entry = gtk4::Entry::new();
    twofa_code_entry.set_input_purpose(gtk4::InputPurpose::Digits);
    twofa_code_entry.set_max_length(6);
    twofa_code_entry.add_css_class("profile-field-entry");
    let twofa_setup_actions = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();
    twofa_setup_actions.add_css_class("profile-actions-row");
    let twofa_confirm_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-2fa-confirm").as_str());
    twofa_confirm_button.add_css_class("suggested-action");
    twofa_confirm_button.add_css_class("profile-action-btn");
    let twofa_cancel_setup_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-2fa-cancel").as_str());
    twofa_cancel_setup_button.add_css_class("flat");
    twofa_cancel_setup_button.add_css_class("profile-action-btn");
    twofa_setup_actions.append(&twofa_confirm_button);
    twofa_setup_actions.append(&twofa_cancel_setup_button);
    for widget in [
        twofa_setup_copy.upcast_ref::<gtk4::Widget>(),
        twofa_qr_picture.upcast_ref::<gtk4::Widget>(),
        twofa_secret_label.upcast_ref::<gtk4::Widget>(),
        twofa_secret_entry.upcast_ref::<gtk4::Widget>(),
        twofa_code_label.upcast_ref::<gtk4::Widget>(),
        twofa_code_entry.upcast_ref::<gtk4::Widget>(),
        twofa_setup_actions.upcast_ref::<gtk4::Widget>(),
    ] {
        twofa_setup_box.append(widget);
    }

    let twofa_enabled_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    let twofa_enabled_copy = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-2fa-enabled-copy").as_str(),
    ));
    twofa_enabled_copy.set_halign(Align::Start);
    twofa_enabled_copy.set_wrap(true);
    twofa_enabled_copy.add_css_class("dim-label");
    twofa_enabled_copy.add_css_class("profile-section-subtitle");
    let twofa_disable_toggle_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-2fa-disable").as_str());
    twofa_disable_toggle_button.add_css_class("flat");
    twofa_disable_toggle_button.add_css_class("profile-action-btn");
    twofa_disable_toggle_button.set_halign(Align::Start);
    let twofa_disable_confirm_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .visible(false)
        .build();
    twofa_disable_confirm_row.add_css_class("profile-actions-row");
    let twofa_disable_confirm_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-2fa-disable-confirm").as_str());
    twofa_disable_confirm_button.add_css_class("suggested-action");
    twofa_disable_confirm_button.add_css_class("profile-action-btn");
    let twofa_disable_cancel_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-2fa-cancel").as_str());
    twofa_disable_cancel_button.add_css_class("flat");
    twofa_disable_cancel_button.add_css_class("profile-action-btn");
    twofa_disable_confirm_row.append(&twofa_disable_confirm_button);
    twofa_disable_confirm_row.append(&twofa_disable_cancel_button);
    twofa_enabled_box.append(&twofa_enabled_copy);
    twofa_enabled_box.append(&twofa_disable_toggle_button);
    twofa_enabled_box.append(&twofa_disable_confirm_row);

    twofa_stack.add_titled(
        &twofa_disabled_box,
        Some("disabled"),
        heelonvault_core::tr!("profile-2fa-stack-disabled").as_str(),
    );
    twofa_stack.add_titled(
        &twofa_setup_box,
        Some("setup"),
        heelonvault_core::tr!("profile-2fa-stack-setup").as_str(),
    );
    twofa_stack.add_titled(
        &twofa_enabled_box,
        Some("enabled"),
        heelonvault_core::tr!("profile-2fa-stack-enabled").as_str(),
    );
    twofa_stack.set_visible_child_name("disabled");

    let twofa_status_label = gtk4::Label::new(None);
    twofa_status_label.set_halign(Align::Start);
    twofa_status_label.set_wrap(true);
    twofa_status_label.add_css_class("inline-status");
    twofa_status_label.add_css_class("profile-inline-status");
    twofa_status_label.set_visible(false);

    twofa_box.append(&twofa_stack);
    twofa_box.append(&twofa_status_label);
    twofa_expander.add_row(&twofa_box);
    twofa_group.add(&twofa_expander);

    TwoFaSection {
        group: twofa_group,
        expander: twofa_expander,
        state_badge: twofa_state_badge,
        stack: twofa_stack,
        disabled_box: twofa_disabled_box,
        disabled_copy: twofa_disabled_copy,
        activate_button: twofa_activate_button,
        setup_box: twofa_setup_box,
        setup_copy: twofa_setup_copy,
        qr_picture: twofa_qr_picture,
        secret_label: twofa_secret_label,
        secret_entry: twofa_secret_entry,
        code_label: twofa_code_label,
        code_entry: twofa_code_entry,
        confirm_button: twofa_confirm_button,
        cancel_setup_button: twofa_cancel_setup_button,
        enabled_box: twofa_enabled_box,
        enabled_copy: twofa_enabled_copy,
        disable_toggle_button: twofa_disable_toggle_button,
        disable_confirm_row: twofa_disable_confirm_row,
        disable_confirm_button: twofa_disable_confirm_button,
        disable_cancel_button: twofa_disable_cancel_button,
        status_label: twofa_status_label,
    }
}

impl TwoFaSection {
    pub fn refresh_i18n(&self) {
        self.expander
            .set_title(heelonvault_core::tr!("profile-section-2fa-title").as_str());
        self.disabled_copy
            .set_text(heelonvault_core::tr!("profile-2fa-disabled-copy").as_str());
        self.activate_button
            .set_label(heelonvault_core::tr!("profile-2fa-activate").as_str());
        self.setup_copy
            .set_text(heelonvault_core::tr!("profile-2fa-setup-copy").as_str());
        self.secret_label
            .set_text(heelonvault_core::tr!("profile-2fa-secret-base32").as_str());
        self.code_label
            .set_text(heelonvault_core::tr!("profile-2fa-code").as_str());
        self.confirm_button
            .set_label(heelonvault_core::tr!("profile-2fa-confirm").as_str());
        self.cancel_setup_button
            .set_label(heelonvault_core::tr!("profile-2fa-cancel").as_str());
        self.enabled_copy
            .set_text(heelonvault_core::tr!("profile-2fa-enabled-copy").as_str());
        self.disable_toggle_button
            .set_label(heelonvault_core::tr!("profile-2fa-disable").as_str());
        self.disable_confirm_button
            .set_label(heelonvault_core::tr!("profile-2fa-disable-confirm").as_str());
        self.disable_cancel_button
            .set_label(heelonvault_core::tr!("profile-2fa-cancel").as_str());
        self.stack
            .page(&self.disabled_box)
            .set_title(heelonvault_core::tr!("profile-2fa-stack-disabled").as_str());
        self.stack
            .page(&self.setup_box)
            .set_title(heelonvault_core::tr!("profile-2fa-stack-setup").as_str());
        self.stack
            .page(&self.enabled_box)
            .set_title(heelonvault_core::tr!("profile-2fa-stack-enabled").as_str());

        MainWindow::set_twofa_badge_state(&self.state_badge, self.is_enabled());
    }

    /// The stack page in front doubles as the authoritative enabled/disabled state.
    pub fn is_enabled(&self) -> bool {
        self.stack
            .visible_child_name()
            .as_ref()
            .is_some_and(|name| name == "enabled")
    }
}

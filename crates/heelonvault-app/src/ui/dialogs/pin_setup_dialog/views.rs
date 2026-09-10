use gtk4::prelude::*;
use gtk4::{Align, InputPurpose, Orientation};
use libadwaita as adw;

use heelonvault_core::services::pin_cache_service::{PIN_MAX_LEN, PIN_MIN_LEN};
use super::types::PinSetupDialogWidgets;

/// Construit l'interface utilisateur complete de la dialogue de configuration du PIN.
///
/// # Arguments
/// * `parent` - La fenetre parente
/// * `pin_currently_active` - Si vrai, le bouton de desactivation PIN est affiche
///
/// # Returns
/// Une structure `PinSetupDialogWidgets` contenant tous les widgets crees.
pub fn build_pin_setup_view(
    parent: &adw::ApplicationWindow,
    pin_currently_active: bool,
) -> PinSetupDialogWidgets {
    // ── Window ────────────────────────────────────────────────────────────
    let window = gtk4::Window::builder()
        .modal(true)
        .transient_for(parent)
        .title(heelonvault_core::tr!("pin-setup-title").as_str())
        .default_width(400)
        .resizable(false)
        .build();
    window.add_css_class("pin-setup-window");

    // ── Root ──────────────────────────────────────────────────────────────
    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();

    // ── Header ───────────────────────────────────────────────────────────
    let header = adw::HeaderBar::new();
    header.set_decoration_layout(Some(":close"));
    window.set_titlebar(Some(&header));

    // ── Body ──────────────────────────────────────────────────────────────
    let body = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(20)
        .margin_bottom(24)
        .margin_start(28)
        .margin_end(28)
        .build();

    // Description
    let desc_label = gtk4::Label::new(Some(heelonvault_core::tr!("pin-setup-description").as_str()));
    desc_label.set_halign(Align::Start);
    desc_label.set_wrap(true);
    desc_label.add_css_class("dim-label");
    body.append(&desc_label);

    // PIN length hint
    let len_hint = gtk4::Label::new(Some(
        &heelonvault_core::i18n::tr_args(
            "pin-setup-length-hint",
            &[
                ("min", heelonvault_core::i18n::I18nArg::Num(PIN_MIN_LEN as i64)),
                ("max", heelonvault_core::i18n::I18nArg::Num(PIN_MAX_LEN as i64)),
            ],
        ),
    ));
    len_hint.set_halign(Align::Start);
    len_hint.set_wrap(true);
    len_hint.add_css_class("caption");
    len_hint.add_css_class("dim-label");
    body.append(&len_hint);

    // New PIN entry
    let pin_entry = gtk4::Entry::new();
    pin_entry.set_visibility(false);
    pin_entry.set_placeholder_text(Some(heelonvault_core::tr!("pin-setup-entry-placeholder").as_str()));
    pin_entry.set_max_length(PIN_MAX_LEN as i32);
    pin_entry.set_hexpand(true);
    pin_entry.set_input_purpose(InputPurpose::Pin);
    pin_entry.add_css_class("pin-digit-entry");
    body.append(&pin_entry);

    // Confirm PIN entry
    let confirm_entry = gtk4::Entry::new();
    confirm_entry.set_visibility(false);
    confirm_entry.set_placeholder_text(Some(heelonvault_core::tr!("pin-setup-confirm-placeholder").as_str()));
    confirm_entry.set_max_length(PIN_MAX_LEN as i32);
    confirm_entry.set_hexpand(true);
    confirm_entry.set_input_purpose(InputPurpose::Pin);
    confirm_entry.add_css_class("pin-digit-entry");
    body.append(&confirm_entry);

    // Feedback label
    let feedback_label = gtk4::Label::new(None);
    feedback_label.set_halign(Align::Start);
    feedback_label.set_wrap(true);
    feedback_label.add_css_class("inline-status");
    feedback_label.add_css_class("pin-feedback-label");
    feedback_label.set_visible(false);
    body.append(&feedback_label);

    // Action buttons row
    let btn_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .hexpand(true)
        .halign(Align::End)
        .build();

    // Save button
    let save_button = gtk4::Button::with_label(heelonvault_core::tr!("pin-setup-save").as_str());
    save_button.add_css_class("suggested-action");
    btn_row.append(&save_button);

    // Disable button (conditional)
    let disable_button = if pin_currently_active {
        let btn = gtk4::Button::with_label(heelonvault_core::tr!("pin-setup-disable").as_str());
        btn.add_css_class("destructive-action");
        btn_row.append(&btn);
        Some(btn)
    } else {
        None
    };

    body.append(&btn_row);
    root.append(&body);
    window.set_child(Some(&root));

    PinSetupDialogWidgets {
        window,
        root,
        header,
        body,
        desc_label,
        len_hint,
        pin_entry,
        confirm_entry,
        feedback_label,
        btn_row,
        save_button,
        disable_button,
    }
}

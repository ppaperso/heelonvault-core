use gtk4::prelude::*;
use gtk4::{Align, InputPurpose, Orientation};
use libadwaita as adw;

use super::types::PinUnlockDialogWidgets;

/// Construit l'interface utilisateur complete de la dialogue de debitage par PIN.
///
/// # Arguments
/// * `parent` - La fenetre parente
///
/// # Returns
/// Une structure `PinUnlockDialogWidgets` contenant tous les widgets crees.
pub fn build_pin_unlock_view(parent: &adw::ApplicationWindow) -> PinUnlockDialogWidgets {
    // ── Window ────────────────────────────────────────────────────────────
    let window = gtk4::Window::builder()
        .modal(true)
        .transient_for(parent)
        .title(heelonvault_core::tr!("pin-unlock-title").as_str())
        .default_width(360)
        .resizable(false)
        .deletable(false)
        .build();
    window.add_css_class("pin-unlock-window");

    // ── Root container ────────────────────────────────────────────────────
    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();

    // Header bar (no close button — user must use PIN or "use master password")
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    header.set_decoration_layout(Some(""));
    window.set_titlebar(Some(&header));

    // ── Body ──────────────────────────────────────────────────────────────
    let body = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(32)
        .margin_end(32)
        .build();

    // Lock icon
    let lock_icon = gtk4::Image::from_icon_name("changes-prevent-symbolic");
    lock_icon.set_pixel_size(48);
    lock_icon.add_css_class("pin-unlock-icon");
    body.append(&lock_icon);

    // Title label
    let title_label = gtk4::Label::new(Some(heelonvault_core::tr!("pin-unlock-title").as_str()));
    title_label.add_css_class("title-2");
    title_label.add_css_class("pin-unlock-heading");
    title_label.set_halign(Align::Center);
    body.append(&title_label);

    // Subtitle
    let subtitle_label =
        gtk4::Label::new(Some(heelonvault_core::tr!("pin-unlock-prompt").as_str()));
    subtitle_label.add_css_class("dim-label");
    subtitle_label.set_halign(Align::Center);
    subtitle_label.set_wrap(true);
    subtitle_label.set_justify(gtk4::Justification::Center);
    body.append(&subtitle_label);

    // PIN entry — digits-only, masked, max 8 chars
    let pin_entry = gtk4::Entry::new();
    pin_entry.set_visibility(false);
    pin_entry.set_placeholder_text(Some(
        heelonvault_core::tr!("pin-unlock-entry-placeholder").as_str(),
    ));
    pin_entry.set_max_length(8);
    pin_entry.set_hexpand(true);
    pin_entry.add_css_class("pin-digit-entry");
    // Hint to on-screen keyboards to show a numeric pad
    pin_entry.set_input_purpose(InputPurpose::Pin);
    body.append(&pin_entry);

    // Inline feedback label (remaining attempts / error)
    let feedback_label = gtk4::Label::new(None);
    feedback_label.set_halign(Align::Center);
    feedback_label.set_wrap(true);
    feedback_label.add_css_class("inline-status");
    feedback_label.add_css_class("pin-feedback-label");
    feedback_label.set_visible(false);
    body.append(&feedback_label);

    // Buttons row
    let btn_row = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .hexpand(true)
        .build();

    let unlock_button =
        gtk4::Button::with_label(heelonvault_core::tr!("pin-unlock-button").as_str());
    unlock_button.add_css_class("suggested-action");
    unlock_button.add_css_class("pill");
    unlock_button.set_hexpand(true);
    btn_row.append(&unlock_button);

    let fallback_button =
        gtk4::Button::with_label(heelonvault_core::tr!("pin-use-master-password").as_str());
    fallback_button.add_css_class("flat");
    fallback_button.add_css_class("pin-fallback-btn");
    fallback_button.set_hexpand(true);
    btn_row.append(&fallback_button);

    let quit_button =
        gtk4::Button::with_label(heelonvault_core::tr!("pin-quit-application").as_str());
    quit_button.add_css_class("flat");
    quit_button.add_css_class("pin-quit-btn");
    quit_button.set_hexpand(true);
    btn_row.append(&quit_button);

    body.append(&btn_row);
    root.append(&body);
    window.set_child(Some(&root));

    PinUnlockDialogWidgets {
        window,
        root,
        header,
        body,
        lock_icon,
        title_label,
        subtitle_label,
        pin_entry,
        feedback_label,
        btn_row,
        unlock_button,
        fallback_button,
        quit_button,
    }
}

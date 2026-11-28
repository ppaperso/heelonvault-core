//! Native login-screen scenario: the real view, production stylesheet, fatal GTK warnings.
//! Driven from the shared GTK smoke entry point (`native_gtk_smoke`).

use gtk4::Orientation;
use libadwaita::prelude::*;

use super::views::{
    LoginDialogWidgets, build_login_view, setup_bootstrap_gates, setup_language_toggle,
};
use crate::ui::license_badge::LicenseDisplay;
use crate::ui::theme_tests::{assert_mapped_text_contrast, settle, snapshot_widget};

/// Canvas, surface and primary-action colours of the 3.0 tokens (`00-tokens.css`).
const CANVAS: &str = "#FAFAFB";
const SURFACE: &str = "#FFFFFF";
const PRIMARY: &str = "#0E706B";

fn present(widgets: &LoginDialogWidgets) -> gtk4::Window {
    let window = gtk4::Window::builder()
        .default_width(480)
        .default_height(680)
        .build();
    window.add_css_class("login-window");
    window.set_titlebar(Some(&widgets.header_bar));
    window.set_child(Some(&widgets.container));
    window.present();
    settle();
    window
}

pub(crate) fn native_login_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let initial_language = heelonvault_core::i18n::current_language();
    assert!(heelonvault_core::i18n::set_language("fr"));
    let license = LicenseDisplay::community();

    // ─── Sign-in ─────────────────────────────────────────────────────────────────
    let widgets = build_login_view(&license, false);
    setup_language_toggle(&widgets, false);
    let window = present(&widgets);

    assert_mapped_text_contrast(&widgets.header_bar, CANVAS)?;
    assert_mapped_text_contrast(&widgets.header_box, CANVAS)?;
    assert_mapped_text_contrast(&widgets.links_box, CANVAS)?;
    assert_mapped_text_contrast(&widgets.credentials_list, SURFACE)?;
    assert_mapped_text_contrast(&widgets.submit_button, PRIMARY)?;
    let (minimum_width, _, _, _) = widgets.shell.measure(Orientation::Horizontal, -1);
    assert!(
        minimum_width <= 440,
        "login content forces {minimum_width}px, wider than the 440px window minimum"
    );
    // Row titles replace the former uppercase field labels.
    assert_eq!(widgets.username_entry.title(), "Identifiant");
    assert_eq!(widgets.password_entry.title(), "Mot de passe");
    snapshot_widget(&window, &window, "login")?;

    // Restoring a database is offered next to the credentials only, not during TOTP.
    assert!(widgets.restore_button.is_visible());
    widgets.step_stack.set_visible_child_name("totp");
    settle();
    assert!(!widgets.restore_button.is_visible());
    assert_mapped_text_contrast(&widgets.totp_step_box, CANVAS)?;
    snapshot_widget(&window, &window, "login-totp")?;
    widgets.step_stack.set_visible_child_name("credentials");
    settle();
    assert!(widgets.restore_button.is_visible());

    // Live retranslation from the title-bar language menu.
    widgets.language_en_button.set_active(true);
    settle();
    assert_eq!(widgets.username_entry.title(), "Username");
    assert_eq!(widgets.language_button.label().as_deref(), Some("English"));
    widgets.language_fr_button.set_active(true);
    settle();
    assert_eq!(widgets.username_entry.title(), "Identifiant");
    assert_eq!(widgets.language_button.label().as_deref(), Some("Français"));
    window.close();

    // ─── First-run wizard ────────────────────────────────────────────────────────
    let wizard = build_login_view(&license, true);
    setup_language_toggle(&wizard, true);
    setup_bootstrap_gates(&wizard);
    let wizard_window = present(&wizard);

    assert_eq!(
        wizard.step_stack.visible_child_name().as_deref(),
        Some("init-identity")
    );
    assert!(wizard.restore_button.parent().is_none());
    assert!(!wizard.submit_button.is_sensitive());
    assert_mapped_text_contrast(&wizard.init_identity_box, CANVAS)?;
    // Synthetic test values only.
    wizard.init_username_entry.set_text("camille");
    wizard.init_password_entry.set_text("short");
    wizard.init_confirm_entry.set_text("short");
    settle();
    assert!(!wizard.submit_button.is_sensitive());
    let strong = "Synthetic-Fixture-Value-47-Orchard!";
    wizard.init_password_entry.set_text(strong);
    wizard.init_confirm_entry.set_text(strong);
    settle();
    assert!(wizard.submit_button.is_sensitive());
    snapshot_widget(&wizard_window, &wizard_window, "login-bootstrap")?;
    wizard_window.close();
    settle();

    assert!(heelonvault_core::i18n::set_language(&initial_language));
    Ok(())
}

//! Views module - UI construction for main window
//!
//! This module contains functions to build the main window UI components.
//!
//! The construction is split into logical sections that can be tested independently.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;

#[allow(unused_imports)]
use super::types::MainWindowWidgets;

/// Build the main application window with basic configuration
pub fn build_main_window(application: &adw::Application) -> adw::ApplicationWindow {
    let initial_launch = super::super::MainWindow::initial_window_launch();
    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("HeelonVault")
        .default_width(initial_launch.width)
        .default_height(initial_launch.height)
        .build();
    window.add_css_class("app-window");
    window.add_css_class("main-window");
    window.set_icon_name(Some("heelonvault"));
    if initial_launch.fullscreen {
        window.fullscreen();
    }
    window
}

/// Build the header bar with title and badges
pub fn build_header_bar(
    license_badge_text: &str,
) -> (
    adw::HeaderBar,
    gtk4::Box,
    gtk4::Image,
    gtk4::Label,
    gtk4::Label,
    gtk4::Widget,
) {
    let header_bar = adw::HeaderBar::new();
    header_bar.add_css_class("main-headerbar");
    header_bar.set_show_start_title_buttons(false);
    header_bar.set_show_end_title_buttons(true);
    header_bar.set_decoration_layout(Some(":close"));

    let title_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();

    // Logo
    let logo =
        gtk4::Image::from_resource("/com/heelonvault/rust/icons/Logo_Heelonys_transparent.png");
    logo.set_pixel_size(22);
    logo.add_css_class("main-title-logo");

    // Title label
    let title_label = gtk4::Label::new(Some("HeelonVault"));
    title_label.add_css_class("title-3");
    title_label.add_css_class("main-title");

    // Version badge
    let header_plan_badge = gtk4::Label::new(Some(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    header_plan_badge.add_css_class("beta-badge");
    header_plan_badge.add_css_class("header-beta-badge");

    // License badge
    let header_license_badge = super::super::header::build_header_license_badge(license_badge_text);

    title_box.append(&logo);
    title_box.append(&title_label);
    title_box.append(&header_plan_badge);
    title_box.append(&header_license_badge);

    header_bar.set_title_widget(Some(&title_box));

    (
        header_bar,
        title_box,
        logo,
        title_label,
        header_plan_badge,
        header_license_badge,
    )
}

/// Build the root container with toast overlay
pub fn build_root_container() -> (gtk4::Box, adw::ToastOverlay) {
    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();
    let toast_overlay = adw::ToastOverlay::new();
    (root, toast_overlay)
}

/// Build the profile button with login history popover
pub fn build_profile_button(
    connected_identity_label: &str,
) -> (gtk4::MenuButton, gtk4::Popover, gtk4::Label, gtk4::Box) {
    let profile_button = gtk4::MenuButton::new();
    profile_button.add_css_class("header-badge");
    profile_button.set_label(
        heelonvault_core::i18n::tr_args(
            "main-connected-label",
            &[(
                "name",
                heelonvault_core::i18n::I18nArg::Str(&connected_identity_label),
            )],
        )
        .as_str(),
    );
    profile_button.set_tooltip_text(Some(
        heelonvault_core::tr!("main-last-logins-tooltip").as_str(),
    ));

    let profile_popover = gtk4::Popover::new();
    profile_popover.set_has_arrow(true);
    profile_popover.set_autohide(true);

    let profile_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();
    profile_box.add_css_class("profile-login-history-popover");

    let profile_title = gtk4::Label::new(Some(
        heelonvault_core::tr!("main-last-logins-title").as_str(),
    ));
    profile_title.set_halign(Align::Start);
    profile_title.add_css_class("profile-login-history-title");
    profile_box.append(&profile_title);

    let login_history_list = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    profile_box.append(&login_history_list);

    profile_popover.set_child(Some(&profile_box));
    profile_button.set_popover(Some(&profile_popover));

    (
        profile_button,
        profile_popover,
        profile_title,
        login_history_list,
    )
}

/// Build the user identity box with profile button and admin badge
pub fn build_user_identity_box(
    profile_button: &gtk4::MenuButton,
    is_admin: bool,
) -> (gtk4::Box, gtk4::Label, gtk4::Button, gtk4::Button) {
    let user_identity_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .build();
    user_identity_box.append(profile_button);

    // Add and trash buttons
    let add_button = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .build();
    add_button.add_css_class("flat");
    add_button.add_css_class("accent");
    add_button.add_css_class("main-add-button");
    add_button.set_tooltip_text(Some(heelonvault_core::tr!("main-add-tooltip").as_str()));

    let trash_button = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .build();
    trash_button.add_css_class("flat");
    trash_button.add_css_class("main-add-button");
    trash_button.set_tooltip_text(Some(heelonvault_core::tr!("main-trash-tooltip").as_str()));

    // Admin badge
    let admin_badge = gtk4::Label::new(Some(heelonvault_core::tr!("main-admin-badge").as_str()));
    if is_admin {
        admin_badge.add_css_class("header-badge");
        admin_badge.add_css_class("admin-badge");
        admin_badge.add_css_class("warning");
        admin_badge.set_tooltip_text(Some(
            heelonvault_core::tr!("main-admin-badge-tooltip").as_str(),
        ));
        user_identity_box.append(&admin_badge);
    }

    (user_identity_box, admin_badge, add_button, trash_button)
}

/// Build the PIN status badge button
pub fn build_pin_status_badge() -> (gtk4::Button, gtk4::Label) {
    let header_pin_label =
        gtk4::Label::new(Some(heelonvault_core::tr!("pin-status-inactive").as_str()));
    header_pin_label.add_css_class("status-role-pill");
    header_pin_label.add_css_class("header-badge");
    header_pin_label.add_css_class("status-role-disabled");

    let header_pin_btn = gtk4::Button::new();
    header_pin_btn.add_css_class("flat");
    header_pin_btn.add_css_class("header-pin-badge");
    header_pin_btn.set_child(Some(&header_pin_label));

    (header_pin_btn, header_pin_label)
}

/// Build the panic button
pub fn build_panic_button() -> (gtk4::Button, gtk4::Label) {
    let panic_lbl = gtk4::Label::new(Some(heelonvault_core::tr!("main-panic-label").as_str()));
    panic_lbl.add_css_class("panic-label");

    let panic_button = gtk4::Button::builder()
        .icon_name("media-playback-stop-symbolic")
        .build();
    panic_button.add_css_class("flat");
    panic_button.add_css_class("panic-button");
    panic_button.set_child(Some(&panic_lbl));
    panic_button.set_tooltip_text(Some(heelonvault_core::tr!("main-panic-tooltip").as_str()));

    (panic_button, panic_lbl)
}

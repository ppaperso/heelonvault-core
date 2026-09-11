//! Admin-only shortcuts to the users and teams pages.

use std::rc::Rc;

use gtk4::Align;
use gtk4::Orientation;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone)]
pub struct AdminSection {
    pub frame: adw::PreferencesGroup,
    subtitle: gtk4::Label,
    users_button: gtk4::Button,
    teams_button: gtk4::Button,
}

/// Build the section and wire its two navigation buttons.
pub fn build(on_open_users_view: Rc<dyn Fn()>, on_open_teams_view: Rc<dyn Fn()>) -> AdminSection {
    let frame = adw::PreferencesGroup::new();
    frame.set_title(heelonvault_core::tr!("profile-section-admin-advanced").as_str());
    frame.add_css_class("profile-section-frame");
    frame.set_hexpand(true);

    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let subtitle = gtk4::Label::new(Some(
        heelonvault_core::tr!("profile-admin-advanced-subtitle").as_str(),
    ));
    subtitle.set_halign(Align::Start);
    subtitle.set_wrap(true);
    subtitle.add_css_class("profile-section-subtitle");
    subtitle.add_css_class("dim-label");

    let actions = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .hexpand(true)
        .halign(Align::End)
        .build();
    actions.add_css_class("profile-actions-row");

    let users_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-admin-open-users").as_str());
    users_button.add_css_class("suggested-action");
    users_button.add_css_class("profile-action-btn");

    let teams_button =
        gtk4::Button::with_label(heelonvault_core::tr!("profile-admin-open-teams").as_str());
    teams_button.add_css_class("suggested-action");
    teams_button.add_css_class("profile-action-btn");

    users_button.connect_clicked(move |_| on_open_users_view());
    teams_button.connect_clicked(move |_| on_open_teams_view());

    actions.append(&users_button);
    actions.append(&teams_button);
    content.append(&subtitle);
    content.append(&actions);
    frame.add(&content);

    AdminSection {
        frame,
        subtitle,
        users_button,
        teams_button,
    }
}

impl AdminSection {
    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-admin-advanced").as_str());
        self.subtitle
            .set_text(heelonvault_core::tr!("profile-admin-advanced-subtitle").as_str());
        self.users_button
            .set_label(heelonvault_core::tr!("profile-admin-open-users").as_str());
        self.teams_button
            .set_label(heelonvault_core::tr!("profile-admin-open-teams").as_str());
    }
}

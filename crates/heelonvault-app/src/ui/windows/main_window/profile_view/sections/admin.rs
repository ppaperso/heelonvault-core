//! Administration entry points. Built from [`Entitlements`] only:
//! - Professional admin: links to the users and teams pages;
//! - Community admin: one locked row describing what the Professional edition adds, with no
//!   action that could reach those features;
//! - regular user: nothing.

use std::rc::Rc;

use gtk4::Align;
use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::ui::entitlements::Entitlements;

#[derive(Clone)]
pub struct AdminSection {
    pub frame: adw::PreferencesGroup,
    edition_pill: Option<gtk4::Label>,
    rows: Vec<(adw::ActionRow, &'static str, &'static str)>,
    learn_more: Option<gtk4::Button>,
}

fn navigation_row(icon_name: &str, on_activate: Rc<dyn Fn()>) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.add_css_class("profile-row-icon");
    row.add_prefix(&icon);
    row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
    row.set_activatable(true);
    row.connect_activated(move |_| on_activate());
    row
}

pub fn build(
    entitlements: Entitlements,
    on_open_users_view: Rc<dyn Fn()>,
    on_open_teams_view: Rc<dyn Fn()>,
) -> Option<AdminSection> {
    let frame = adw::PreferencesGroup::new();
    frame.add_css_class("profile-group");

    let section = if entitlements.can_manage_users() {
        let pill = gtk4::Label::new(None);
        pill.add_css_class("profile-pill");
        pill.add_css_class("profile-pill-professional");
        pill.set_valign(Align::Center);
        frame.set_header_suffix(Some(&pill));
        let users = navigation_row("hv-user-symbolic", on_open_users_view);
        let teams = navigation_row("hv-team-symbolic", on_open_teams_view);
        frame.add(&users);
        frame.add(&teams);
        AdminSection {
            frame,
            edition_pill: Some(pill),
            rows: vec![
                (
                    users,
                    "profile-admin-open-users",
                    "profile-admin-users-subtitle",
                ),
                (
                    teams,
                    "profile-admin-open-teams",
                    "profile-admin-teams-subtitle",
                ),
            ],
            learn_more: None,
        }
    } else if entitlements.shows_professional_offer() {
        let row = adw::ActionRow::new();
        row.add_css_class("profile-offer-row");
        let icon = gtk4::Image::from_icon_name("hv-lock-symbolic");
        icon.add_css_class("profile-row-icon");
        row.add_prefix(&icon);
        let learn_more = gtk4::Button::new();
        learn_more.add_css_class("flat");
        learn_more.add_css_class("profile-row-button");
        learn_more.set_valign(Align::Center);
        learn_more.connect_clicked(|_| {
            let _ = gio::AppInfo::launch_default_for_uri(
                crate::constants::PRODUCT_URL,
                None::<&gio::AppLaunchContext>,
            );
        });
        row.add_suffix(&learn_more);
        frame.add(&row);
        AdminSection {
            frame,
            edition_pill: None,
            rows: vec![(
                row,
                "profile-admin-offer-title",
                "profile-admin-offer-subtitle",
            )],
            learn_more: Some(learn_more),
        }
    } else {
        return None;
    };
    section.refresh_i18n();
    Some(section)
}

impl AdminSection {
    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-admin-advanced").as_str());
        if let Some(pill) = &self.edition_pill {
            pill.set_text(heelonvault_core::tr!("profile-edition-professional-short").as_str());
        }
        for (row, title, subtitle) in &self.rows {
            row.set_title(heelonvault_core::i18n::tr(title).as_str());
            row.set_subtitle(heelonvault_core::i18n::tr(subtitle).as_str());
        }
        if let Some(button) = &self.learn_more {
            button.set_label(heelonvault_core::tr!("profile-admin-offer-learn-more").as_str());
        }
    }
}

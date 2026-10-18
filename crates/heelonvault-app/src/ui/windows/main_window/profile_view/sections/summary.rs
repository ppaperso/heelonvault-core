//! Page header: who is signed in, with which role and edition, and how well the account is
//! protected. The protection checklist only states facts the app controls (2FA, PIN,
//! auto-lock); it never claims the account is "safe".

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use heelonvault_core::i18n::{I18nArg, tr, tr_args};
use libadwaita as adw;

use crate::ui::entitlements::{Edition, Entitlements};

const PROTECTIONS: [&str; 3] = [
    "profile-protection-2fa",
    "profile-protection-pin",
    "profile-protection-auto-lock",
];

#[derive(Clone)]
struct ProtectionItem {
    icon: gtk4::Image,
    label: gtk4::Label,
}

#[derive(Clone)]
pub struct SummarySection {
    pub frame: gtk4::Box,
    avatar: adw::Avatar,
    name: gtk4::Label,
    account: gtk4::Label,
    role: gtk4::Label,
    edition: gtk4::Label,
    entitlements: Entitlements,
    protection_title: gtk4::Label,
    protection_hint: gtk4::Label,
    protection_level: gtk4::LevelBar,
    items: Vec<ProtectionItem>,
    state: std::rc::Rc<std::cell::Cell<[bool; 3]>>,
}

fn pill(class: &str) -> gtk4::Label {
    let label = gtk4::Label::new(None);
    label.add_css_class("profile-pill");
    label.add_css_class(class);
    label.set_valign(Align::Center);
    label
}

pub fn build(entitlements: Entitlements) -> SummarySection {
    let frame = gtk4::Box::new(Orientation::Vertical, 16);
    frame.add_css_class("profile-summary");

    let identity = gtk4::Box::new(Orientation::Horizontal, 16);
    let avatar = adw::Avatar::new(56, None, true);
    identity.append(&avatar);
    let names = gtk4::Box::new(Orientation::Vertical, 2);
    names.set_hexpand(true);
    names.set_valign(Align::Center);
    let name = gtk4::Label::new(None);
    name.set_xalign(0.0);
    name.add_css_class("title-3");
    name.add_css_class("profile-summary-name");
    let account = gtk4::Label::new(None);
    account.set_xalign(0.0);
    account.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    account.add_css_class("dim-label");
    names.append(&name);
    names.append(&account);
    identity.append(&names);
    let role = pill(if entitlements.is_admin {
        "profile-pill-admin"
    } else {
        "profile-pill-user"
    });
    let edition = pill(match entitlements.edition {
        Edition::Professional => "profile-pill-professional",
        Edition::Community => "profile-pill-community",
    });
    identity.append(&role);
    identity.append(&edition);
    frame.append(&identity);

    let protection = gtk4::Box::new(Orientation::Vertical, 8);
    protection.add_css_class("profile-protection");
    let header = gtk4::Box::new(Orientation::Horizontal, 12);
    let protection_title = gtk4::Label::new(None);
    protection_title.set_xalign(0.0);
    protection_title.set_hexpand(true);
    protection_title.add_css_class("heading");
    let protection_level = gtk4::LevelBar::for_interval(0.0, 3.0);
    protection_level.set_mode(gtk4::LevelBarMode::Discrete);
    protection_level.set_width_request(96);
    protection_level.set_valign(Align::Center);
    protection_level.add_css_class("profile-protection-level");
    header.append(&protection_title);
    header.append(&protection_level);
    protection.append(&header);

    let checklist = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .min_children_per_line(1)
        .max_children_per_line(3)
        .column_spacing(18)
        .row_spacing(6)
        .build();
    let items: Vec<_> = PROTECTIONS
        .iter()
        .map(|key| {
            let item = gtk4::Box::new(Orientation::Horizontal, 6);
            let icon = gtk4::Image::from_icon_name("hv-warning-symbolic");
            icon.set_pixel_size(16);
            let label = gtk4::Label::new(Some(&tr(key)));
            item.append(&icon);
            item.append(&label);
            checklist.insert(&item, -1);
            ProtectionItem { icon, label }
        })
        .collect();
    protection.append(&checklist);
    let protection_hint = gtk4::Label::new(None);
    protection_hint.set_xalign(0.0);
    protection_hint.set_wrap(true);
    protection_hint.add_css_class("dim-label");
    protection_hint.add_css_class("caption");
    protection.append(&protection_hint);
    frame.append(&protection);

    let section = SummarySection {
        frame,
        avatar,
        name,
        account,
        role,
        edition,
        entitlements,
        protection_title,
        protection_hint,
        protection_level,
        items,
        state: std::rc::Rc::new(std::cell::Cell::new([false; 3])),
    };
    section.refresh_i18n();
    section
}

impl SummarySection {
    pub fn set_identity(&self, username: &str, display_name: Option<&str>, email: Option<&str>) {
        let display = display_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(username);
        self.name.set_text(display);
        self.avatar.set_text(Some(display));
        let account = match email.filter(|value| !value.trim().is_empty()) {
            Some(email) => format!("{username} · {email}"),
            None => username.to_string(),
        };
        self.account.set_text(&account);
    }

    /// `[2FA, PIN, auto-lock]`, in the order of the checklist.
    pub fn set_protection(&self, state: [bool; 3]) {
        self.state.set(state);
        self.render_protection();
    }

    fn render_protection(&self) {
        let state = self.state.get();
        let active = state.iter().filter(|enabled| **enabled).count();
        self.protection_title.set_text(&tr_args(
            "profile-protection-title",
            &[("count", I18nArg::Num(active as i64))],
        ));
        self.protection_level.set_value(active as f64);
        self.protection_hint.set_text(&tr(if active == state.len() {
            "profile-protection-hint-complete"
        } else {
            "profile-protection-hint-missing"
        }));
        for (item, enabled) in self.items.iter().zip(state) {
            item.icon.set_icon_name(Some(if enabled {
                "hv-check-symbolic"
            } else {
                "hv-warning-symbolic"
            }));
            for (class, on) in [
                ("profile-protection-on", enabled),
                ("profile-protection-off", !enabled),
            ] {
                if on {
                    item.icon.add_css_class(class);
                } else {
                    item.icon.remove_css_class(class);
                }
            }
        }
    }

    pub fn refresh_i18n(&self) {
        self.role.set_text(&tr(if self.entitlements.is_admin {
            "profile-role-admin"
        } else {
            "profile-role-user"
        }));
        self.edition.set_text(&tr(match self.entitlements.edition {
            Edition::Professional => "license-status-professional",
            Edition::Community => "license-status-community",
        }));
        for (item, key) in self.items.iter().zip(PROTECTIONS) {
            item.label.set_text(&tr(key));
        }
        self.render_protection();
    }
}

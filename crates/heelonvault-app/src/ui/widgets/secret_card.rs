use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Align, Button, Label, Orientation, Separator};
use uuid::Uuid;

fn build_action_button(icon_candidates: &[&str], fallback_glyph: &str, tooltip: &str) -> Button {
    let button = Button::new();
    button.add_css_class("flat");
    button.add_css_class("secret-card-action-btn");
    button.set_tooltip_text(Some(tooltip));
    button.set_hexpand(true);

    let resolved_icon = gtk4::gdk::Display::default().and_then(|display| {
        let theme = gtk4::IconTheme::for_display(&display);
        icon_candidates
            .iter()
            .find(|name| theme.has_icon(name))
            .map(|name| (*name).to_string())
    });

    if let Some(icon_name) = resolved_icon {
        let image = gtk4::Image::from_icon_name(&icon_name);
        image.add_css_class("secret-card-action-icon");
        button.set_child(Some(&image));
    } else {
        let glyph = Label::new(Some(fallback_glyph));
        glyph.add_css_class("secret-card-action-glyph");
        button.set_child(Some(&glyph));
    }

    button
}

/// Represents a secret for display purposes
// Phase 5a migration: several fields are written but not yet read (UI wiring incomplete).
// Owner: ppaadmin | Due: Phase 5b | Tracked: Open Core Phase 5b milestone
#[allow(dead_code)]
#[derive(Clone)]
pub struct SecretRowData {
    pub secret_id: Uuid,
    pub icon_name: String,
    pub type_label: String,
    pub title: String,
    pub created_at: String,
    pub login: String,
    pub url: String,
    pub secret_value: String,
    pub color_class: String,
    // Mock fields for badges (will be linked to DB later)
    pub health: String, // "Robuste" or "Faible"
    pub is_health_access: bool,
    pub usage_count: u32,   // Number of times copied
    pub is_duplicate: bool, // Whether password is reused
    pub is_incomplete: bool,
    pub is_shared_vault: bool,
    pub can_edit: bool,
    pub can_delete: bool,
    /// When non-empty, show a vault badge (used during cross-vault search).
    pub vault_name: String,
}

/// A modern card widget for displaying a secret item.
///
/// Layout (top → bottom):
///   header_row : [title]
///   info_strip : "login · domain"  (hidden when both are empty)
///   badges_box : health · usage · duplicate? · shared? · vault?
///   separator
///   actions_box: [🔑 copy_password] [👤 copy_login?] [🌐 open_url?]
#[allow(dead_code)]
pub struct SecretCard {
    card_box: gtk4::Box,
    secret_id: Uuid,
    copy_button: Button,
    copy_login_button: Option<Button>,
    open_url_button: Option<Button>,
    usage_badge: Label,
}

impl SecretCard {
    pub fn new(data: SecretRowData) -> Self {
        let card_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(0)
            .margin_bottom(0)
            .margin_start(0)
            .margin_end(0)
            .build();
        card_box.set_size_request(-1, -1);
        card_box.set_vexpand(false);
        card_box.set_hexpand(false);
        card_box.set_valign(Align::Start);
        card_box.add_css_class("secret-card");
        card_box.add_css_class("card");
        card_box.add_css_class(data.color_class.as_str());

        // --- HEADER ROW : title ---
        let header_row = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .build();
        header_row.add_css_class("secret-card-header");

        let title_label = Label::new(Some(&data.title));
        title_label.set_halign(Align::Start);
        title_label.set_hexpand(true);
        title_label.set_wrap(false);
        title_label.set_ellipsize(EllipsizeMode::End);
        title_label.set_single_line_mode(true);
        title_label.add_css_class("secret-card-title");
        title_label.add_css_class("heading");

        header_row.append(&title_label);

        // --- INFO STRIP : "login · domain" ---
        // Extract domain from URL without an external crate: strip scheme, take up to first '/'.
        let domain: String = if data.url.is_empty() {
            String::new()
        } else {
            let stripped = data
                .url
                .trim_start_matches("https://")
                .trim_start_matches("http://");
            stripped.split('/').next().unwrap_or("").to_string()
        };
        let strip_text = match (data.login.is_empty(), domain.is_empty()) {
            (false, false) => format!("{} · {}", data.login, domain),
            (false, true) => data.login.clone(),
            (true, false) => domain.clone(),
            (true, true) => String::new(),
        };
        let info_strip = Label::new(Some(&strip_text));
        info_strip.set_halign(Align::Start);
        info_strip.set_hexpand(true);
        info_strip.set_wrap(false);
        info_strip.set_ellipsize(EllipsizeMode::End);
        info_strip.set_single_line_mode(true);
        // Reserve height even when hidden so layout stays stable across card sizes.
        info_strip.set_size_request(-1, 20);
        info_strip.set_visible(!strip_text.is_empty());
        info_strip.add_css_class("secret-card-info-strip");

        // --- BADGES ROW ---
        let badges_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();
        badges_box.add_css_class("secret-card-badges");

        // Health badge
        let health_badge = Label::new(Some(&data.health));
        health_badge.set_single_line_mode(true);
        health_badge.add_css_class("secret-badge");
        if data.health.to_lowercase().contains("faible") {
            health_badge.add_css_class("badge-weak");
        } else {
            health_badge.add_css_class("badge-strong");
        }
        badges_box.append(&health_badge);

        if data.is_health_access {
            let access_badge = Label::new(Some("Sante"));
            access_badge.set_single_line_mode(true);
            access_badge.add_css_class("secret-badge");
            access_badge.add_css_class("badge-health");
            badges_box.append(&access_badge);
        }

        // Usage badge
        let usage_badge = Label::new(Some(&format!("↗ {}", data.usage_count)));
        usage_badge.set_single_line_mode(true);
        usage_badge.add_css_class("secret-badge");
        usage_badge.add_css_class("badge-usage");
        badges_box.append(&usage_badge);

        // Duplicate badge
        if data.is_duplicate {
            let dup_badge = Label::new(Some(
                heelonvault_core::tr!("secret-card-duplicate-badge").as_str(),
            ));
            dup_badge.set_single_line_mode(true);
            dup_badge.add_css_class("secret-badge");
            dup_badge.add_css_class("badge-duplicate");
            badges_box.append(&dup_badge);
        }

        // Incomplete badge: guide users to fill both login and URL metadata.
        if data.is_incomplete {
            let incomplete_badge = Label::new(Some("Incomplet"));
            incomplete_badge.set_single_line_mode(true);
            incomplete_badge.add_css_class("secret-badge");
            incomplete_badge.add_css_class("badge-incomplete");
            badges_box.append(&incomplete_badge);
        }

        if data.is_shared_vault {
            let shared_badge = Label::new(Some("Partagé"));
            shared_badge.set_single_line_mode(true);
            shared_badge.add_css_class("secret-badge");
            shared_badge.add_css_class("badge-usage");
            badges_box.append(&shared_badge);
        }

        // Vault badge: shown during cross-vault search to identify the origin vault
        if !data.vault_name.is_empty() {
            let vault_badge = Label::new(Some(&format!("🗄 {}", data.vault_name)));
            vault_badge.set_single_line_mode(true);
            vault_badge.set_ellipsize(EllipsizeMode::End);
            vault_badge.add_css_class("secret-badge");
            vault_badge.add_css_class("badge-vault");
            badges_box.append(&vault_badge);
        }

        // --- SEPARATOR ---
        let separator = Separator::new(gtk4::Orientation::Horizontal);
        separator.add_css_class("secret-card-separator");

        // --- ACTIONS BOX (Quick-actions — daily use) ---
        // Layout: [🔑 copy_password] [👤 copy_login?] [🌐 open_url?]
        // copy_login and open_url are conditionally visible based on data.
        let actions_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .homogeneous(false)
            .build();
        actions_box.add_css_class("secret-card-actions");

        // 🔑 Copy password — always present; disabled only if secret_value is empty.
        let copy_button = build_action_button(
            &["edit-copy-symbolic", "document-duplicate-symbolic"],
            "⧉",
            "Copier le mot de passe",
        );
        copy_button.set_sensitive(!data.secret_value.is_empty());
        actions_box.append(&copy_button);

        // 👤 Copy login — visible only when a login is stored.
        let copy_login_button: Option<Button> = if !data.login.is_empty() {
            let btn = build_action_button(
                &["avatar-default-symbolic", "system-users-symbolic"],
                "@",
                "Copier le login",
            );
            actions_box.append(&btn);
            Some(btn)
        } else {
            None
        };

        // 🌐 Open URL — visible only when a URL is stored.
        let open_url_button: Option<Button> = if !data.url.is_empty() {
            let btn = build_action_button(
                &[
                    "help-browser-symbolic",
                    "web-browser-symbolic",
                    "edit-find-symbolic",
                ],
                "↗",
                "Ouvrir dans le navigateur",
            );
            actions_box.append(&btn);
            Some(btn)
        } else {
            None
        };

        // --- ASSEMBLE CARD ---
        card_box.append(&header_row);
        card_box.append(&info_strip);
        card_box.append(&badges_box);
        card_box.append(&separator);
        card_box.append(&actions_box);

        Self {
            card_box,
            secret_id: data.secret_id,
            copy_button,
            copy_login_button,
            open_url_button,
            usage_badge,
        }
    }

    pub fn get_widget(&self) -> gtk4::Box {
        self.card_box.clone()
    }

    #[allow(dead_code)]
    pub fn get_secret_id(&self) -> Uuid {
        self.secret_id
    }

    pub fn update_usage_count(&self, new_count: u32) {
        self.usage_badge.set_label(&format!("↗ {}", new_count));
    }

    pub fn get_copy_button(&self) -> Button {
        self.copy_button.clone()
    }

    pub fn get_copy_login_button(&self) -> Option<Button> {
        self.copy_login_button.clone()
    }

    pub fn get_open_url_button(&self) -> Option<Button> {
        self.open_url_button.clone()
    }
}

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Align, Button, Label, Orientation, Separator};
use uuid::Uuid;

pub use crate::ui::view_preferences::SecretViewMode;

#[allow(dead_code)]
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

/// One "icon + text" line of the card info box (login or domain).
///
/// `width_chars` pins the label width so list rows line up as columns; an empty `text`
/// then keeps its slot (invisible icon, blank label) instead of shifting the next column.
fn build_info_row(
    icon_name: &str,
    text: &str,
    label_class: &str,
    width_chars: Option<i32>,
) -> gtk4::Box {
    let row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.add_css_class("secret-card-info-icon");
    icon.set_pixel_size(16);
    icon.set_valign(Align::Center);
    if text.is_empty() {
        icon.set_opacity(0.0);
    }
    row.append(&icon);

    let label = Label::new(Some(text));
    label.set_halign(Align::Start);
    label.set_xalign(0.0);
    label.set_ellipsize(EllipsizeMode::End);
    label.set_single_line_mode(true);
    label.set_valign(Align::Center);
    match width_chars {
        Some(chars) => {
            label.set_width_chars(chars);
            label.set_max_width_chars(chars);
        }
        None => label.set_hexpand(true),
    }
    label.add_css_class("secret-card-info-label");
    label.add_css_class(label_class);
    row.append(&label);

    row
}

/// Host part of a URL, without an external crate: strip scheme, take up to first '/'.
fn extract_domain(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Translated label of the password strength badge (also indexed by the search).
pub fn strength_label(is_weak: bool) -> String {
    if is_weak {
        heelonvault_core::tr!("main-strength-weak")
    } else {
        heelonvault_core::tr!("main-strength-strong")
    }
}

/// Width, in characters, of the title / login / domain columns of the list layout.
const LIST_TITLE_CHARS: i32 = 24;
const LIST_INFO_CHARS: i32 = 22;
/// Fixed width of the actions column, room for the three quick actions.
const LIST_ACTIONS_WIDTH: i32 = 132;

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
    /// Password strength verdict; the badge label is translated at display time.
    pub is_weak: bool,
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

/// A widget displaying one secret item, as a grid card or as a compact list row.
///
/// Grid layout (top → bottom):
///   header_row : [title]
///   info_box : [login] [domain] (each on its own line, hidden when both empty)
///   badges_box : health · usage · duplicate? · shared? · vault?
///   separator
///   actions_box: [🔑 copy_password] [👤 copy_login?] [🌐 open_url?]
///
/// List layout (left → right, fixed-width columns so rows line up):
///   [type icon] [title] [login] [domain] [health · duplicate? · vault?] [actions]
#[allow(dead_code)]
pub struct SecretCard {
    card_box: gtk4::Box,
    secret_id: Uuid,
    copy_button: Button,
    copy_login_button: Option<Button>,
    open_url_button: Option<Button>,
    usage_badge: Label,
}

/// Quick-action buttons shared by both layouts.
struct CardActions {
    actions_box: gtk4::Box,
    copy_button: Button,
    copy_login_button: Option<Button>,
    open_url_button: Option<Button>,
}

#[allow(dead_code)]
impl SecretCard {
    pub fn new(data: SecretRowData, mode: SecretViewMode) -> Self {
        let domain = if data.url.is_empty() {
            String::new()
        } else {
            extract_domain(&data.url)
        };
        let title_label = Self::build_title(&data.title);
        let compact = mode == SecretViewMode::List;
        let (badges_box, usage_badge) = Self::build_badges(&data, compact);
        let actions = Self::build_actions(&data);

        let card_box = match mode {
            SecretViewMode::Grid => {
                Self::assemble_grid(&data, &domain, title_label, badges_box, &actions)
            }
            SecretViewMode::List => {
                Self::assemble_row(&data, &domain, title_label, badges_box, &actions)
            }
        };

        Self {
            card_box,
            secret_id: data.secret_id,
            copy_button: actions.copy_button,
            copy_login_button: actions.copy_login_button,
            open_url_button: actions.open_url_button,
            usage_badge,
        }
    }

    fn build_title(title: &str) -> Label {
        let title_label = Label::new(Some(title));
        title_label.set_halign(Align::Start);
        title_label.set_xalign(0.0);
        title_label.set_wrap(false);
        title_label.set_ellipsize(EllipsizeMode::End);
        title_label.set_single_line_mode(true);
        title_label.add_css_class("secret-card-title");
        title_label.add_css_class("heading");
        title_label
    }

    fn assemble_grid(
        data: &SecretRowData,
        domain: &str,
        title_label: Label,
        badges_box: gtk4::Box,
        actions: &CardActions,
    ) -> gtk4::Box {
        let card_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .build();
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
        title_label.set_hexpand(true);
        header_row.append(&title_label);

        // --- INFO BOX : login (line 1) + domain (line 2) ---
        let info_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(2)
            .build();
        info_box.add_css_class("secret-card-info-box");
        if !data.login.is_empty() {
            info_box.append(&build_info_row(
                "avatar-default-symbolic",
                &data.login,
                "secret-card-login",
                None,
            ));
        }
        if !domain.is_empty() {
            info_box.append(&build_info_row(
                "applications-internet-symbolic",
                domain,
                "secret-card-domain",
                None,
            ));
        }
        info_box.set_visible(!data.login.is_empty() || !domain.is_empty());

        // --- SEPARATOR ---
        let separator = Separator::new(gtk4::Orientation::Horizontal);
        separator.add_css_class("secret-card-separator");

        card_box.append(&header_row);
        card_box.append(&info_box);
        card_box.append(&badges_box);
        card_box.append(&separator);
        card_box.append(&actions.actions_box);
        card_box
    }

    fn assemble_row(
        data: &SecretRowData,
        domain: &str,
        title_label: Label,
        badges_box: gtk4::Box,
        actions: &CardActions,
    ) -> gtk4::Box {
        let row_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(14)
            .build();
        row_box.set_hexpand(true);
        row_box.set_valign(Align::Center);
        row_box.add_css_class("secret-card");
        row_box.add_css_class("secret-card-row");
        row_box.add_css_class("card");
        row_box.add_css_class(data.color_class.as_str());

        let type_icon = gtk4::Image::from_icon_name(&data.icon_name);
        type_icon.set_pixel_size(20);
        type_icon.set_valign(Align::Center);
        type_icon.set_tooltip_text(Some(&data.type_label));
        type_icon.add_css_class("secret-card-row-icon");
        row_box.append(&type_icon);

        title_label.set_width_chars(LIST_TITLE_CHARS);
        title_label.set_max_width_chars(LIST_TITLE_CHARS);
        title_label.set_valign(Align::Center);
        row_box.append(&title_label);

        row_box.append(&build_info_row(
            "avatar-default-symbolic",
            &data.login,
            "secret-card-login",
            Some(LIST_INFO_CHARS),
        ));
        row_box.append(&build_info_row(
            "applications-internet-symbolic",
            domain,
            "secret-card-domain",
            Some(LIST_INFO_CHARS),
        ));

        // Only flexible column: every column before it has a fixed width, so rows line up.
        badges_box.set_hexpand(true);
        badges_box.set_valign(Align::Center);
        badges_box.add_css_class("secret-card-row-badges");
        row_box.append(&badges_box);

        // Fixed-width, left-packed actions: "copy password" sits at the same x on every row.
        let actions_box = &actions.actions_box;
        actions_box.set_size_request(LIST_ACTIONS_WIDTH, -1);
        actions_box.set_valign(Align::Center);
        actions_box.add_css_class("secret-card-row-actions");
        let mut button = actions_box.first_child();
        while let Some(child) = button {
            child.set_hexpand(false);
            button = child.next_sibling();
        }
        row_box.append(actions_box);

        row_box
    }

    /// Status badges. `compact` (list layout) keeps only the risk-relevant ones; the usage
    /// badge is still built so `update_usage_count` stays valid, just not displayed.
    fn build_badges(data: &SecretRowData, compact: bool) -> (gtk4::Box, Label) {
        let badges_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();
        badges_box.add_css_class("secret-card-badges");

        // Health badge
        let health_badge = Label::new(Some(strength_label(data.is_weak).as_str()));
        health_badge.set_single_line_mode(true);
        health_badge.add_css_class("secret-badge");
        health_badge.add_css_class(if data.is_weak {
            "badge-weak"
        } else {
            "badge-strong"
        });
        badges_box.append(&health_badge);

        if data.is_health_access && !compact {
            let access_badge = Label::new(Some(
                heelonvault_core::tr!("secret-card-health-badge").as_str(),
            ));
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
        if !compact {
            badges_box.append(&usage_badge);
        }

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
        if data.is_incomplete && !compact {
            let incomplete_badge = Label::new(Some(
                heelonvault_core::tr!("secret-card-incomplete-badge").as_str(),
            ));
            incomplete_badge.set_single_line_mode(true);
            incomplete_badge.add_css_class("secret-badge");
            incomplete_badge.add_css_class("badge-incomplete");
            badges_box.append(&incomplete_badge);
        }

        if data.is_shared_vault && !compact {
            let shared_badge = Label::new(Some(
                heelonvault_core::tr!("secret-card-shared-badge").as_str(),
            ));
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
            if compact {
                vault_badge.set_max_width_chars(18);
            }
            vault_badge.add_css_class("secret-badge");
            vault_badge.add_css_class("badge-vault");
            badges_box.append(&vault_badge);
        }

        (badges_box, usage_badge)
    }

    /// Quick actions (daily use): [🔑 copy_password] [👤 copy_login?] [🌐 open_url?].
    /// copy_login and open_url are conditionally present based on data.
    fn build_actions(data: &SecretRowData) -> CardActions {
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
            heelonvault_core::tr!("secret-card-copy-password-tooltip").as_str(),
        );
        copy_button.set_sensitive(!data.secret_value.is_empty());
        actions_box.append(&copy_button);

        // 👤 Copy login — visible only when a login is stored.
        let copy_login_button: Option<Button> = if !data.login.is_empty() {
            let btn = build_action_button(
                &["avatar-default-symbolic", "system-users-symbolic"],
                "@",
                heelonvault_core::tr!("secret-card-copy-login-tooltip").as_str(),
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
                heelonvault_core::tr!("secret-card-open-url-tooltip").as_str(),
            );
            actions_box.append(&btn);
            Some(btn)
        } else {
            None
        };

        CardActions {
            actions_box,
            copy_button,
            copy_login_button,
            open_url_button,
        }
    }

    #[allow(dead_code)]
    pub fn get_widget(&self) -> gtk4::Box {
        self.card_box.clone()
    }

    #[allow(dead_code)]
    pub fn get_secret_id(&self) -> Uuid {
        self.secret_id
    }

    #[allow(dead_code)]
    pub fn update_usage_count(&self, new_count: u32) {
        self.usage_badge.set_label(&format!("↗ {}", new_count));
    }

    #[allow(dead_code)]
    pub fn get_copy_button(&self) -> Button {
        self.copy_button.clone()
    }

    #[allow(dead_code)]
    pub fn get_copy_login_button(&self) -> Option<Button> {
        self.copy_login_button.clone()
    }

    #[allow(dead_code)]
    pub fn get_open_url_button(&self) -> Option<Button> {
        self.open_url_button.clone()
    }
}

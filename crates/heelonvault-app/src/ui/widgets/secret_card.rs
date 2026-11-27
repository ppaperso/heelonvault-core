use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Align, Button, Label, Orientation, Separator};
use uuid::Uuid;

pub use crate::ui::view_preferences::SecretViewMode;

/// Window action filling the search field with a `category:`/`tag:` query.
/// Registered as `search-classification` on the main window (`window/core.rs`).
pub(crate) const SEARCH_CLASSIFICATION_ACTION: &str = "win.search-classification";
/// Tags shown on a grid card / list row before a "+N" badge.
const GRID_VISIBLE_TAGS: usize = 3;
/// The list row has one flexible column: only the category fits, tags go into "+N".
const LIST_VISIBLE_TAGS: usize = 0;
/// Status badges shown on a grid card before a "+N" badge, the most at-risk first.
const GRID_VISIBLE_BADGES: usize = 2;
/// The grid title wraps on this many lines before being ellipsized.
const GRID_TITLE_LINES: i32 = 2;

/// Search query matching one category or tag. The search splits on whitespace and
/// its normalization reads `-` as a space, so `client a` becomes `tag:client-a`.
pub(crate) fn classification_query(field: &str, value: &str) -> String {
    format!(
        "{field}:{}",
        value.split_whitespace().collect::<Vec<_>>().join("-")
    )
}

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

/// Search run by the passphrase badge (`type:` field, see `search_filter`).
pub const PASSPHRASE_QUERY: &str = "type:passphrase";

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
    /// The secret has a value to copy. The value itself is never handed to the widget.
    pub has_secret: bool,
    pub is_password: bool,
    /// Password item stored as a passphrase (metadata `password_kind`).
    pub is_passphrase: bool,
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
    /// Category and comma-separated tags (metadata), shown as clickable badges.
    pub category: String,
    pub tags: String,
}

/// "Copied N times", translated at display time.
fn usage_tooltip(count: u32) -> String {
    heelonvault_core::i18n::tr_args(
        "secret-card-usage-tooltip",
        &[(
            "count",
            heelonvault_core::i18n::I18nArg::Num(i64::from(count)),
        )],
    )
}

/// A widget displaying one secret item, as a grid card or as a compact list row.
///
/// Grid layout (top → bottom), every card of a grid has the same size:
///   header_row : [type icon] [title, up to two lines] [copies]
///   info_box : [login] [domain] [vault?] (each on its own line, hidden when all empty)
///   chips : passphrase? · category · tags
///   badges_box : two status badges at most, the most at-risk first, then "+N"
///   separator
///   actions_box: [🔑 copy_password] [👤 copy_login?] [🌐 open_url?] (right-aligned)
///
/// List layout (left → right, fixed-width columns so rows line up):
///   [type icon] [title] [login] [domain] [copies · health · duplicate? · vault?] [actions]
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
        let (usage_box, usage_badge) = Self::build_usage_badge(data.usage_count);
        let actions = Self::build_actions(&data);

        let card_box = match mode {
            SecretViewMode::Grid => {
                Self::assemble_grid(&data, &domain, title_label, usage_box, &actions)
            }
            SecretViewMode::List => {
                let badges_box = Self::build_row_badges(&data, &usage_box);
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
        usage_box: gtk4::Box,
        actions: &CardActions,
    ) -> gtk4::Box {
        let card_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();
        // Fill the (homogeneous) grid cell, so every card of a grid has the same height.
        card_box.set_vexpand(true);
        card_box.set_hexpand(false);
        card_box.set_valign(Align::Fill);
        card_box.add_css_class("secret-card");
        card_box.add_css_class("secret-card-grid");
        card_box.add_css_class("card");
        card_box.add_css_class(data.color_class.as_str());

        // --- HEADER ROW : type icon, title (two lines), copy counter ---
        let header_row = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .build();
        header_row.add_css_class("secret-card-header");
        let type_icon = gtk4::Image::from_icon_name(&data.icon_name);
        type_icon.set_pixel_size(20);
        type_icon.set_valign(Align::Start);
        type_icon.set_tooltip_text(Some(&data.type_label));
        type_icon.add_css_class("secret-card-type-icon");
        header_row.append(&type_icon);
        // The title is what users scan for: wrap it rather than cut it after a few letters.
        title_label.set_single_line_mode(false);
        title_label.set_wrap(true);
        title_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        title_label.set_lines(GRID_TITLE_LINES);
        title_label.set_width_chars(14);
        title_label.set_max_width_chars(26);
        title_label.set_valign(Align::Start);
        title_label.set_hexpand(true);
        title_label.set_tooltip_text(Some(&data.title));
        header_row.append(&title_label);
        usage_box.set_valign(Align::Start);
        usage_box.add_css_class("secret-card-usage");
        header_row.append(&usage_box);

        // --- INFO BOX : login, domain, vault (one line each) ---
        let info_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(2)
            .build();
        info_box.add_css_class("secret-card-info-box");
        let lines = [
            ("hv-user-symbolic", data.login.as_str(), "secret-card-login"),
            ("hv-globe-symbolic", domain, "secret-card-domain"),
            // Origin vault, set during a cross-vault search.
            (
                "hv-vault-symbolic",
                data.vault_name.as_str(),
                "secret-card-vault",
            ),
        ];
        for (icon, text, class) in lines {
            if !text.is_empty() {
                info_box.append(&build_info_row(icon, text, class, None));
            }
        }
        info_box.set_visible(info_box.first_child().is_some());

        let separator = Separator::new(gtk4::Orientation::Horizontal);
        separator.add_css_class("secret-card-separator");
        // Pushes the separator and the actions to the bottom of the card.
        let spacer = gtk4::Box::new(Orientation::Vertical, 0);
        spacer.set_vexpand(true);

        card_box.append(&header_row);
        card_box.append(&info_box);
        if let Some(chips) = Self::build_classification(data, GRID_VISIBLE_TAGS) {
            card_box.append(&chips);
        }
        if let Some(badges_box) = Self::build_grid_badges(data) {
            card_box.append(&badges_box);
        }
        card_box.append(&spacer);
        card_box.append(&separator);
        let actions_box = &actions.actions_box;
        actions_box.set_halign(Align::End);
        let mut button = actions_box.first_child();
        while let Some(child) = button {
            child.set_hexpand(false);
            button = child.next_sibling();
        }
        card_box.append(actions_box);
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
            "hv-user-symbolic",
            &data.login,
            "secret-card-login",
            Some(LIST_INFO_CHARS),
        ));
        row_box.append(&build_info_row(
            "hv-globe-symbolic",
            domain,
            "secret-card-domain",
            Some(LIST_INFO_CHARS),
        ));

        // Only flexible column: every column before it has a fixed width, so rows line up.
        if let Some(chips) = Self::build_classification(data, LIST_VISIBLE_TAGS) {
            badges_box.append(&chips);
        }
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

    /// One clickable badge running the matching search.
    fn build_chip(field: &str, value: &str, category: bool) -> Button {
        let button = Self::build_query_chip(value, classification_query(field, value));
        if category {
            button.add_css_class("secret-card-chip-category");
        }
        button
    }

    /// Badge marking a passphrase; a click lists every passphrase.
    fn build_passphrase_chip() -> Button {
        let button = Self::build_query_chip(
            &heelonvault_core::tr!("secret-type-passphrase"),
            PASSPHRASE_QUERY.to_string(),
        );
        button.add_css_class("secret-card-chip-passphrase");
        button
    }

    fn build_query_chip(value: &str, query: String) -> Button {
        let label = Label::new(Some(value));
        label.set_single_line_mode(true);
        label.set_ellipsize(EllipsizeMode::End);
        label.set_max_width_chars(14);
        let button = Button::new();
        button.set_child(Some(&label));
        button.add_css_class("flat");
        button.add_css_class("secret-card-chip");
        button.set_tooltip_text(Some(&heelonvault_core::i18n::tr_args(
            "secret-card-chip-search",
            &[("query", heelonvault_core::i18n::I18nArg::Str(&query))],
        )));
        button.connect_clicked(move |button| {
            use gtk4::glib::prelude::ToVariant;
            // Absent outside the main window (e.g. isolated widget tests): nothing to do.
            if let Err(error) =
                button.activate_action(SEARCH_CLASSIFICATION_ACTION, Some(&query.to_variant()))
            {
                tracing::debug!(%error, "classification search action unavailable");
            }
        });
        button
    }

    /// Category then tags, `visible_tags` at most followed by a "+N" badge listing
    /// the others. `None` when the secret has neither.
    fn build_classification(data: &SecretRowData, visible_tags: usize) -> Option<gtk4::Box> {
        let category = data.category.trim();
        let tags = crate::ui::widgets::classification::parse_tags(&data.tags);
        if category.is_empty() && tags.is_empty() && !data.is_passphrase {
            return None;
        }
        let chips = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .build();
        chips.add_css_class("secret-card-chips");
        if data.is_passphrase {
            chips.append(&Self::build_passphrase_chip());
        }
        if !category.is_empty() {
            chips.append(&Self::build_chip("category", category, true));
        }
        for tag in tags.iter().take(visible_tags) {
            chips.append(&Self::build_chip("tag", tag, false));
        }
        if tags.len() > visible_tags {
            let hidden = &tags[visible_tags..];
            let more = Label::new(Some(&heelonvault_core::i18n::tr_args(
                "secret-card-chip-more",
                &[(
                    "count",
                    heelonvault_core::i18n::I18nArg::Num(hidden.len() as i64),
                )],
            )));
            more.add_css_class("secret-card-chip-more");
            more.set_tooltip_text(Some(&hidden.join(", ")));
            chips.append(&more);
        }
        Some(chips)
    }

    /// Copy counter: copy icon and count, "Copied N times" as tooltip. Returns the badge and
    /// its count label (updated in place by `update_usage_count`).
    fn build_usage_badge(count: u32) -> (gtk4::Box, Label) {
        let badge = gtk4::Box::new(Orientation::Horizontal, 4);
        badge.add_css_class("secret-badge");
        badge.add_css_class("badge-usage");
        badge.set_valign(Align::Center);
        let icon = gtk4::Image::from_icon_name("hv-copy-symbolic");
        icon.set_pixel_size(12);
        let label = Label::new(Some(&count.to_string()));
        label.set_single_line_mode(true);
        label.add_css_class("badge-usage-count");
        badge.append(&icon);
        badge.append(&label);
        badge.set_tooltip_text(Some(&usage_tooltip(count)));
        (badge, label)
    }

    /// One status badge.
    fn status_label(text: &str, class: &str) -> Label {
        let badge = Label::new(Some(text));
        badge.set_single_line_mode(true);
        badge.add_css_class("secret-badge");
        badge.add_css_class(class);
        badge
    }

    /// Strength verdict and its badge class, `None` for non-password items.
    fn strength_status(data: &SecretRowData) -> Option<(String, &'static str)> {
        if !data.is_password {
            return None;
        }
        Some(if !data.has_secret {
            (
                heelonvault_core::tr!("security-password-unavailable"),
                "dim-label",
            )
        } else if data.is_weak {
            (strength_label(true), "badge-weak")
        } else {
            (strength_label(false), "badge-strong")
        })
    }

    /// Grid status badges, the most at-risk first: weak, duplicate, incomplete, then the
    /// informative ones.
    fn grid_statuses(data: &SecretRowData) -> Vec<(String, &'static str)> {
        let strength = Self::strength_status(data);
        let (risky_strength, calm_strength) = match strength {
            Some(status) if status.1 == "badge-weak" => (Some(status), None),
            other => (None, other),
        };
        let mut statuses: Vec<(String, &'static str)> = Vec::new();
        statuses.extend(risky_strength);
        if data.is_duplicate {
            statuses.push((
                heelonvault_core::tr!("secret-card-duplicate-badge"),
                "badge-duplicate",
            ));
        }
        if data.is_incomplete {
            statuses.push((
                heelonvault_core::tr!("secret-card-incomplete-badge"),
                "badge-incomplete",
            ));
        }
        statuses.extend(calm_strength);
        if data.is_shared_vault {
            statuses.push((
                heelonvault_core::tr!("secret-card-shared-badge"),
                "badge-usage",
            ));
        }
        if data.is_health_access {
            statuses.push((
                heelonvault_core::tr!("secret-card-health-badge"),
                "badge-health",
            ));
        }
        statuses
    }

    /// Grid badge row: `GRID_VISIBLE_BADGES` at most, the others folded into "+N" with
    /// their names as tooltip, so the row never pushes the card wider.
    fn build_grid_badges(data: &SecretRowData) -> Option<gtk4::Box> {
        let statuses = Self::grid_statuses(data);
        if statuses.is_empty() {
            return None;
        }
        let badges_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();
        badges_box.add_css_class("secret-card-badges");
        for (text, class) in statuses.iter().take(GRID_VISIBLE_BADGES) {
            badges_box.append(&Self::status_label(text, class));
        }
        if statuses.len() > GRID_VISIBLE_BADGES {
            let hidden: Vec<&str> = statuses[GRID_VISIBLE_BADGES..]
                .iter()
                .map(|(text, _)| text.as_str())
                .collect();
            let more = Label::new(Some(&heelonvault_core::i18n::tr_args(
                "secret-card-chip-more",
                &[(
                    "count",
                    heelonvault_core::i18n::I18nArg::Num(hidden.len() as i64),
                )],
            )));
            more.add_css_class("secret-card-chip-more");
            more.add_css_class("secret-card-badges-more");
            more.set_tooltip_text(Some(&hidden.join(", ")));
            badges_box.append(&more);
        }
        Some(badges_box)
    }

    /// List badges: the copy counter first, so it lines up on every row, then only the
    /// risk-relevant badges.
    fn build_row_badges(data: &SecretRowData, usage_box: &gtk4::Box) -> gtk4::Box {
        let badges_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .build();
        badges_box.add_css_class("secret-card-badges");
        badges_box.append(usage_box);
        if let Some((text, class)) = Self::strength_status(data) {
            badges_box.append(&Self::status_label(&text, class));
        }
        if data.is_duplicate {
            badges_box.append(&Self::status_label(
                &heelonvault_core::tr!("secret-card-duplicate-badge"),
                "badge-duplicate",
            ));
        }
        // Vault badge: shown during cross-vault search to identify the origin vault. A plain
        // label (no icon) so it can shrink with the row's only flexible column.
        if !data.vault_name.is_empty() {
            let vault_badge = Self::status_label(&data.vault_name, "badge-vault");
            vault_badge.set_ellipsize(EllipsizeMode::End);
            vault_badge.set_max_width_chars(18);
            badges_box.append(&vault_badge);
        }
        badges_box
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
            &["hv-copy-symbolic", "document-duplicate-symbolic"],
            "⧉",
            heelonvault_core::tr!("secret-card-copy-password-tooltip").as_str(),
        );
        copy_button.set_sensitive(data.has_secret);
        actions_box.append(&copy_button);

        // 👤 Copy login — visible only when a login is stored.
        let copy_login_button: Option<Button> = if !data.login.is_empty() {
            let btn = build_action_button(
                &["hv-user-symbolic", "hv-user-symbolic"],
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
                &["hv-globe-symbolic", "web-browser-symbolic"],
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
        self.usage_badge.set_label(&new_count.to_string());
        if let Some(badge) = self.usage_badge.parent() {
            badge.set_tooltip_text(Some(&usage_tooltip(new_count)));
        }
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

/// Native GTK check of the classification badges (run from `native_gtk_smoke`).
#[cfg(test)]
pub(crate) fn native_card_classification_scenario() -> Result<(), Box<dyn std::error::Error>> {
    use std::cell::RefCell;
    use std::rc::Rc;

    fn data(category: &str, tags: &str) -> SecretRowData {
        SecretRowData {
            secret_id: Uuid::new_v4(),
            icon_name: "hv-key-symbolic".into(),
            type_label: "Password".into(),
            title: "Demo".into(),
            created_at: String::new(),
            login: "demo".into(),
            url: String::new(),
            has_secret: true,
            is_password: true,
            is_passphrase: false,
            color_class: "secret-type-password".into(),
            is_weak: false,
            is_health_access: false,
            usage_count: 0,
            is_duplicate: false,
            is_incomplete: false,
            is_shared_vault: false,
            can_edit: true,
            can_delete: true,
            vault_name: String::new(),
            category: category.into(),
            tags: tags.into(),
        }
    }
    fn collect(widget: &gtk4::Widget, class: &str, found: &mut Vec<gtk4::Widget>) {
        if widget.has_css_class(class) {
            found.push(widget.clone());
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            collect(&current, class, found);
            child = current.next_sibling();
        }
    }

    // The card resolves `win.*` through its ancestors, like inside the main window.
    let host = gtk4::Box::new(Orientation::Vertical, 0);
    let actions = gtk4::gio::SimpleActionGroup::new();
    let received = Rc::new(RefCell::new(Vec::<String>::new()));
    let action =
        gtk4::gio::SimpleAction::new("search-classification", Some(gtk4::glib::VariantTy::STRING));
    let sink = Rc::clone(&received);
    action.connect_activate(move |_, query| {
        if let Some(query) = query.and_then(|value| value.get::<String>()) {
            sink.borrow_mut().push(query);
        }
    });
    actions.add_action(&action);
    host.insert_action_group("win", Some(&actions));
    // Rooted like the real cards, so action lookup walks a complete hierarchy.
    let window = gtk4::Window::new();
    window.set_child(Some(&host));

    let tags = "prod, client a, finance, demo, legacy";
    for (mode, visible) in [
        (SecretViewMode::Grid, GRID_VISIBLE_TAGS),
        (SecretViewMode::List, LIST_VISIBLE_TAGS),
    ] {
        let card = SecretCard::new(data("Travail", tags), mode);
        host.append(&card.get_widget());
        let mut chips = Vec::new();
        collect(
            card.get_widget().upcast_ref(),
            "secret-card-chip",
            &mut chips,
        );
        assert_eq!(chips.len(), 1 + visible, "category + visible tags");
        // The copy counter is shown in both layouts and follows live updates.
        let mut usage = Vec::new();
        collect(card.get_widget().upcast_ref(), "badge-usage", &mut usage);
        let usage = usage.first().ok_or("missing copy counter")?;
        assert_eq!(
            usage.tooltip_text().as_deref(),
            Some(usage_tooltip(0).as_str())
        );
        card.update_usage_count(3);
        let mut counts = Vec::new();
        collect(usage, "badge-usage-count", &mut counts);
        let count = counts
            .first()
            .and_then(|widget| widget.clone().downcast::<Label>().ok())
            .ok_or("missing copy count")?;
        assert_eq!(count.text(), "3");
        assert_eq!(
            usage.tooltip_text().as_deref(),
            Some(usage_tooltip(3).as_str())
        );
        let mut more = Vec::new();
        collect(
            card.get_widget().upcast_ref(),
            "secret-card-chip-more",
            &mut more,
        );
        let more = more
            .first()
            .and_then(|widget| widget.clone().downcast::<Label>().ok())
            .ok_or("missing +N badge")?;
        assert_eq!(
            more.text(),
            heelonvault_core::i18n::tr_args(
                "secret-card-chip-more",
                &[(
                    "count",
                    heelonvault_core::i18n::I18nArg::Num((5 - visible) as i64)
                )],
            )
        );
        received.borrow_mut().clear();
        for chip in &chips {
            chip.clone()
                .downcast::<Button>()
                .map_err(|_| "chip is not a button")?
                .emit_clicked();
        }
        let expected: Vec<String> = std::iter::once("category:Travail".to_string())
            .chain(
                ["tag:prod", "tag:client-a", "tag:finance"]
                    .into_iter()
                    .take(visible)
                    .map(String::from),
            )
            .collect();
        assert_eq!(*received.borrow(), expected);
        host.remove(&card.get_widget());
    }

    // Grid card: the copy counter lives in the header, the title wraps on two lines and
    // only the two most at-risk statuses are shown, the rest folded into "+N".
    let mut risky = data("", "");
    risky.title = "A long title that would be cut after a few letters on one line".into();
    risky.is_weak = true;
    risky.is_duplicate = true;
    risky.is_incomplete = true;
    risky.is_shared_vault = true;
    let card = SecretCard::new(risky, SecretViewMode::Grid);
    let widget: gtk4::Widget = card.get_widget().upcast();
    let mut headers = Vec::new();
    collect(&widget, "secret-card-header", &mut headers);
    let header = headers.first().ok_or("missing grid header")?;
    let mut header_usage = Vec::new();
    collect(header, "badge-usage", &mut header_usage);
    assert_eq!(header_usage.len(), 1, "copy counter in the header");
    let mut titles = Vec::new();
    collect(header, "secret-card-title", &mut titles);
    let title = titles
        .first()
        .and_then(|widget| widget.clone().downcast::<Label>().ok())
        .ok_or("missing title")?;
    assert!(title.wraps());
    assert_eq!(title.lines(), GRID_TITLE_LINES);
    assert_eq!(
        title.tooltip_text().as_deref(),
        Some(title.text().as_str()),
        "full title as tooltip"
    );
    let mut rows = Vec::new();
    collect(&widget, "secret-card-badges", &mut rows);
    let row = rows.first().ok_or("missing badge row")?;
    let mut statuses = Vec::new();
    collect(row, "secret-badge", &mut statuses);
    let texts: Vec<String> = statuses
        .iter()
        .filter_map(|widget| widget.clone().downcast::<Label>().ok())
        .map(|label| label.text().to_string())
        .collect();
    assert_eq!(
        texts,
        vec![
            strength_label(true),
            heelonvault_core::tr!("secret-card-duplicate-badge")
        ],
        "most at-risk statuses first"
    );
    let mut folded = Vec::new();
    collect(row, "secret-card-badges-more", &mut folded);
    let folded = folded.first().ok_or("missing +N status badge")?;
    assert_eq!(
        folded.tooltip_text().as_deref(),
        Some(
            format!(
                "{}, {}",
                heelonvault_core::tr!("secret-card-incomplete-badge"),
                heelonvault_core::tr!("secret-card-shared-badge")
            )
            .as_str()
        )
    );

    window.set_child(gtk4::Widget::NONE);
    window.destroy();
    let plain = SecretCard::new(data("", ""), SecretViewMode::Grid);
    let mut none = Vec::new();
    collect(
        plain.get_widget().upcast_ref(),
        "secret-card-chips",
        &mut none,
    );
    assert!(none.is_empty(), "no badge row without category or tags");
    Ok(())
}

#[cfg(test)]
mod classification_tests {
    use super::*;

    #[test]
    fn badge_queries_survive_the_whitespace_tokenizer() {
        assert_eq!(classification_query("tag", "prod"), "tag:prod");
        assert_eq!(classification_query("tag", " client  a "), "tag:client-a");
        assert_eq!(
            classification_query("category", "Travail"),
            "category:Travail"
        );
    }

    #[test]
    fn badge_keys_exist_in_both_catalogs() {
        for lang in ["fr", "en"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../heelonvault-core/locales")
                .join(lang)
                .join("main.ftl");
            let catalog = std::fs::read_to_string(&path).unwrap_or_default();
            for key in ["secret-card-chip-search", "secret-card-chip-more"] {
                assert!(
                    catalog
                        .lines()
                        .any(|line| line.split_once(" =").is_some_and(|(id, _)| id == key)),
                    "{key} missing in {lang}"
                );
            }
        }
    }

    #[test]
    fn the_action_name_matches_the_window_registration() {
        let core = include_str!("../windows/main_window/window/core.rs");
        let name = SEARCH_CLASSIFICATION_ACTION
            .strip_prefix("win.")
            .unwrap_or_default();
        assert!(
            core.contains(&format!("\"{name}\"")),
            "action {name} not registered"
        );
    }
}

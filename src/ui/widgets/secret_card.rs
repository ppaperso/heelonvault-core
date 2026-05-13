use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Align, Button, Label, Orientation, Separator};
use uuid::Uuid;

/// Represents a secret for display purposes
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
    pub health: String,     // "Robuste" or "Faible"
    pub usage_count: u32,   // Number of times copied
    pub is_duplicate: bool, // Whether password is reused
    pub is_shared_vault: bool,
    pub can_edit: bool,
    pub can_delete: bool,
}

/// A modern card widget for displaying a secret item
pub struct SecretCard {
    card_box: gtk4::Box,
    secret_id: Uuid,
    edit_button: Button,
    copy_button: Button,
    trash_button: Button,
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

        // --- TITLE (Top) ---
        let title_label = Label::new(Some(&data.title));
        title_label.set_halign(Align::Start);
        title_label.set_wrap(false);
        title_label.set_ellipsize(EllipsizeMode::End);
        title_label.set_single_line_mode(true);
        title_label.add_css_class("secret-card-title");
        title_label.add_css_class("heading");

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

        // Usage badge
        let usage_badge = Label::new(Some(&format!("↗ {}", data.usage_count)));
        usage_badge.set_single_line_mode(true);
        usage_badge.add_css_class("secret-badge");
        usage_badge.add_css_class("badge-usage");
        badges_box.append(&usage_badge);

        // Duplicate badge
        if data.is_duplicate {
            let dup_badge = Label::new(Some(crate::tr!("secret-card-duplicate-badge").as_str()));
            dup_badge.set_single_line_mode(true);
            dup_badge.add_css_class("secret-badge");
            dup_badge.add_css_class("badge-duplicate");
            badges_box.append(&dup_badge);
        }

        if data.is_shared_vault {
            let shared_badge = Label::new(Some("Partagé"));
            shared_badge.set_single_line_mode(true);
            shared_badge.add_css_class("secret-badge");
            shared_badge.add_css_class("badge-usage");
            badges_box.append(&shared_badge);
        }

        // --- SEPARATOR ---
        let separator = Separator::new(gtk4::Orientation::Horizontal);
        separator.add_css_class("secret-card-separator");

        // --- ACTIONS BOX (Footer) ---
        let actions_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .homogeneous(true)
            .build();
        actions_box.add_css_class("secret-card-actions");

        let copy_button = Button::builder().icon_name("edit-copy-symbolic").build();
        copy_button.add_css_class("flat");
        copy_button.add_css_class("secret-card-action-btn");
        copy_button.set_tooltip_text(Some("Copier"));
        copy_button.set_hexpand(true);

        let edit_button = Button::builder()
            .icon_name("document-edit-symbolic")
            .build();
        edit_button.add_css_class("flat");
        edit_button.add_css_class("secret-card-action-btn");
        edit_button.set_tooltip_text(Some("Modifier"));
        edit_button.set_hexpand(true);
        edit_button.set_sensitive(data.can_edit);

        let trash_button = Button::builder().icon_name("user-trash-symbolic").build();
        trash_button.add_css_class("flat");
        trash_button.add_css_class("secret-card-action-btn");
        trash_button.set_tooltip_text(Some("Corbeille"));
        trash_button.set_hexpand(true);
        trash_button.set_sensitive(data.can_delete);

        actions_box.append(&copy_button);
        actions_box.append(&edit_button);
        actions_box.append(&trash_button);

        // --- ASSEMBLE CARD ---
        card_box.append(&title_label);
        card_box.append(&badges_box);
        card_box.append(&separator);
        card_box.append(&actions_box);

        Self {
            card_box,
            secret_id: data.secret_id,
            edit_button,
            copy_button,
            trash_button,
            usage_badge,
        }
    }

    pub fn get_widget(&self) -> gtk4::Box {
        self.card_box.clone()
    }

    pub fn get_secret_id(&self) -> Uuid {
        self.secret_id
    }

    pub fn update_usage_count(&self, new_count: u32) {
        self.usage_badge.set_label(&format!("↗ {}", new_count));
    }

    pub fn get_edit_button(&self) -> Button {
        self.edit_button.clone()
    }

    pub fn get_copy_button(&self) -> Button {
        self.copy_button.clone()
    }

    pub fn get_trash_button(&self) -> Button {
        self.trash_button.clone()
    }
}

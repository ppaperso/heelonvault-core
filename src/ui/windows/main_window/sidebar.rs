use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use uuid::Uuid;

use super::SidebarWidgets;

pub(super) fn build_sidebar_panel() -> SidebarWidgets {
    let sidebar_frame = gtk4::Frame::new(None);
    sidebar_frame.add_css_class("main-sidebar");

    let sidebar_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(14)
        .margin_bottom(14)
        .margin_start(14)
        .margin_end(14)
        .build();

    let my_vaults_title = gtk4::Label::new(Some(crate::tr!("main-my-vaults-title").as_str()));
    my_vaults_title.add_css_class("main-section-title");
    my_vaults_title.set_halign(Align::Start);

    let create_vault_button = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .build();
    create_vault_button.add_css_class("flat");
    create_vault_button.add_css_class("accent");
    create_vault_button.set_tooltip_text(Some(crate::tr!("main-create-vault-button").as_str()));

    let my_vaults_header = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .build();
    my_vaults_header.append(&my_vaults_title);
    my_vaults_header.append(&create_vault_button);
    sidebar_box.append(&my_vaults_header);

    let my_vaults_list = gtk4::ListBox::new();
    my_vaults_list.add_css_class("boxed-list");
    my_vaults_list.add_css_class("main-category-list");
    my_vaults_list.set_selection_mode(gtk4::SelectionMode::Single);
    sidebar_box.append(&my_vaults_list);

    let shared_vaults_title =
        gtk4::Label::new(Some(crate::tr!("main-shared-with-me-title").as_str()));
    shared_vaults_title.add_css_class("main-section-title");
    shared_vaults_title.set_halign(Align::Start);
    shared_vaults_title.set_visible(false);

    let shared_vaults_list = gtk4::ListBox::new();
    shared_vaults_list.add_css_class("boxed-list");
    shared_vaults_list.add_css_class("main-category-list");
    shared_vaults_list.set_selection_mode(gtk4::SelectionMode::Single);
    shared_vaults_list.set_visible(false);
    sidebar_box.append(&shared_vaults_title);
    sidebar_box.append(&shared_vaults_list);

    let vaults_separator = gtk4::Separator::new(Orientation::Horizontal);
    sidebar_box.append(&vaults_separator);

    let audit_title = gtk4::Label::new(Some(crate::tr!("main-audit-title").as_str()));
    audit_title.add_css_class("main-section-title");
    audit_title.set_halign(Align::Start);
    sidebar_box.append(&audit_title);

    let audit_list = gtk4::ListBox::new();
    audit_list.add_css_class("boxed-list");
    audit_list.add_css_class("main-audit-list");
    audit_list.set_selection_mode(gtk4::SelectionMode::Single);

    let (audit_all_row, audit_all_label, audit_all_badge) =
        build_audit_sidebar_row(crate::tr!("main-audit-all").as_str(), "view-grid-symbolic");
    let (audit_weak_row, audit_weak_label, audit_weak_badge) = build_audit_sidebar_row(
        crate::tr!("main-audit-weak").as_str(),
        "dialog-warning-symbolic",
    );
    let (audit_duplicate_row, audit_duplicate_label, audit_duplicate_badge) =
        build_audit_sidebar_row(
            crate::tr!("main-audit-duplicates").as_str(),
            "content-copy-symbolic",
        );
    audit_list.append(&audit_all_row);
    audit_list.append(&audit_weak_row);
    audit_list.append(&audit_duplicate_row);
    audit_list.select_row(Some(&audit_all_row));
    sidebar_box.append(&audit_list);

    let certification_menu_button = gtk4::MenuButton::new();
    certification_menu_button.add_css_class("flat");
    certification_menu_button.add_css_class("sidebar-profile-entry");
    certification_menu_button.set_halign(Align::Fill);
    certification_menu_button.set_hexpand(true);
    certification_menu_button.set_icon_name("emblem-ok-symbolic");
    certification_menu_button.set_label("Certifier & Exporter");
    certification_menu_button.set_tooltip_text(Some("Certifier & Exporter"));
    sidebar_box.append(&certification_menu_button);

    let sidebar_title = gtk4::Label::new(Some(crate::tr!("main-categories-title").as_str()));
    sidebar_title.add_css_class("main-section-title");
    sidebar_title.set_halign(Align::Start);
    sidebar_box.append(&sidebar_title);

    let category_list = gtk4::ListBox::new();
    category_list.add_css_class("boxed-list");
    category_list.add_css_class("main-category-list");
    category_list.set_selection_mode(gtk4::SelectionMode::Single);

    let rows = [
        (crate::tr!("main-category-all"), "view-grid-symbolic"),
        (
            crate::tr!("main-category-passwords"),
            "dialog-password-symbolic",
        ),
        (
            crate::tr!("main-category-api-tokens"),
            "dialog-key-symbolic",
        ),
        (
            crate::tr!("main-category-ssh-keys"),
            "network-wired-symbolic",
        ),
        (
            crate::tr!("main-category-documents"),
            "folder-documents-symbolic",
        ),
    ];

    let mut category_labels: Vec<gtk4::Label> = Vec::new();
    for (index, (title, icon_name)) in rows.into_iter().enumerate() {
        let (row, label) = build_sidebar_row(title.as_str(), icon_name);
        category_labels.push(label);
        category_list.append(&row);
        if index == 0 {
            category_list.select_row(Some(&row));
        }
    }

    sidebar_box.append(&category_list);

    let account_title = gtk4::Label::new(Some(crate::tr!("main-account-title").as_str()));
    account_title.add_css_class("main-section-title");
    account_title.set_halign(Align::Start);
    sidebar_box.append(&account_title);

    let profile_security_button = gtk4::Button::new();
    profile_security_button.add_css_class("flat");
    profile_security_button.add_css_class("sidebar-profile-entry");
    profile_security_button.set_halign(Align::Fill);
    let profile_security_inner = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(10)
        .margin_end(10)
        .build();
    let profile_security_icon = gtk4::Image::from_icon_name("preferences-system-symbolic");
    profile_security_icon.set_pixel_size(18);
    profile_security_icon.add_css_class("main-sidebar-icon");
    let profile_security_label =
        gtk4::Label::new(Some(crate::tr!("main-profile-security").as_str()));
    profile_security_label.add_css_class("main-sidebar-label");
    profile_security_label.set_halign(Align::Start);
    profile_security_label.set_hexpand(true);
    profile_security_inner.append(&profile_security_icon);
    profile_security_inner.append(&profile_security_label);
    profile_security_button.set_child(Some(&profile_security_inner));
    sidebar_box.append(&profile_security_button);

    let teams_button = gtk4::Button::new();
    teams_button.add_css_class("flat");
    teams_button.add_css_class("sidebar-profile-entry");
    teams_button.set_halign(Align::Fill);
    let teams_inner = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(10)
        .margin_end(10)
        .build();
    let teams_icon = gtk4::Image::from_icon_name("system-users-symbolic");
    teams_icon.set_pixel_size(18);
    teams_icon.add_css_class("main-sidebar-icon");
    let teams_label = gtk4::Label::new(Some(crate::tr!("main-teams-nav").as_str()));
    teams_label.add_css_class("main-sidebar-label");
    teams_label.set_halign(Align::Start);
    teams_label.set_hexpand(true);
    teams_inner.append(&teams_icon);
    teams_inner.append(&teams_label);
    teams_button.set_child(Some(&teams_inner));
    sidebar_box.append(&teams_button);

    let administration_button = gtk4::Button::new();
    administration_button.add_css_class("flat");
    administration_button.add_css_class("sidebar-profile-entry");
    administration_button.set_halign(Align::Fill);
    let administration_inner = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(10)
        .margin_end(10)
        .build();
    let administration_icon = gtk4::Image::from_icon_name("avatar-default-symbolic");
    administration_icon.set_pixel_size(18);
    administration_icon.add_css_class("main-sidebar-icon");
    let administration_label = gtk4::Label::new(Some(crate::tr!("main-user-nav").as_str()));
    administration_label.add_css_class("main-sidebar-label");
    administration_label.set_halign(Align::Start);
    administration_label.set_hexpand(true);
    administration_inner.append(&administration_icon);
    administration_inner.append(&administration_label);
    administration_button.set_child(Some(&administration_inner));
    sidebar_box.append(&administration_button);
    administration_button.set_visible(false);

    sidebar_frame.set_child(Some(&sidebar_box));
    SidebarWidgets {
        frame: sidebar_frame,
        my_vaults_title,
        create_vault_button,
        my_vaults_list,
        shared_vaults_title,
        shared_vaults_list,
        category_list,
        audit_list,
        audit_title,
        categories_title: sidebar_title,
        account_title,
        audit_all_label,
        audit_weak_label,
        audit_duplicate_label,
        category_all_label: category_labels[0].clone(),
        category_passwords_label: category_labels[1].clone(),
        category_api_tokens_label: category_labels[2].clone(),
        category_ssh_keys_label: category_labels[3].clone(),
        category_documents_label: category_labels[4].clone(),
        audit_all_badge,
        audit_weak_badge,
        audit_duplicate_badge,
        certification_menu_button,
        profile_security_label,
        profile_security_button,
        teams_label,
        teams_button,
        administration_label,
        administration_button,
    }
}

pub(super) fn build_audit_sidebar_row(
    title: &str,
    icon_name: &str,
) -> (gtk4::ListBoxRow, gtk4::Label, gtk4::Label) {
    let row = gtk4::ListBoxRow::new();
    let content = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(10)
        .margin_end(10)
        .build();

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_pixel_size(18);
    icon.add_css_class("main-sidebar-icon");
    content.append(&icon);

    let label = gtk4::Label::new(Some(title));
    label.set_halign(Align::Start);
    label.set_hexpand(true);
    label.add_css_class("main-sidebar-label");
    content.append(&label);

    let badge = gtk4::Label::new(Some("0"));
    badge.add_css_class("audit-count-badge");
    content.append(&badge);

    row.set_child(Some(&content));
    (row, label, badge)
}

pub(super) fn build_sidebar_row(title: &str, icon_name: &str) -> (gtk4::ListBoxRow, gtk4::Label) {
    let row = gtk4::ListBoxRow::new();
    let content = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(10)
        .margin_end(10)
        .build();

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_pixel_size(18);
    icon.add_css_class("main-sidebar-icon");
    content.append(&icon);

    let label = gtk4::Label::new(Some(title));
    label.set_halign(Align::Start);
    label.set_hexpand(true);
    label.add_css_class("main-sidebar-label");
    content.append(&label);

    row.set_child(Some(&content));
    (row, label)
}

pub(super) fn build_vault_sidebar_row(
    title: &str,
    vault_id: Uuid,
    can_delete: bool,
    is_shared_with_others: bool,
    shared_role: Option<crate::models::VaultShareRole>,
    secret_count: usize,
    on_delete: Option<Rc<dyn Fn(Uuid, String)>>,
) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    let content = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(10)
        .margin_end(10)
        .build();

    let icon = gtk4::Image::from_icon_name("folder-symbolic");
    icon.set_pixel_size(18);
    icon.add_css_class("main-sidebar-icon");
    content.append(&icon);

    let label = gtk4::Label::new(Some(title));
    label.set_halign(Align::Start);
    label.set_hexpand(true);
    label.add_css_class("main-sidebar-label");
    content.append(&label);

    if shared_role.is_some() || is_shared_with_others {
        let shared_icon = gtk4::Image::from_icon_name("emblem-shared-symbolic");
        shared_icon.set_pixel_size(14);
        shared_icon.add_css_class("main-sidebar-icon");
        shared_icon.add_css_class("vault-shared-indicator");
        shared_icon.set_tooltip_text(Some(crate::tr!("main-vault-shared-tooltip").as_str()));
        content.append(&shared_icon);
    }

    if let Some(role) = shared_role {
        let role_badge = gtk4::Label::new(None);
        let badge_text = match role {
            crate::models::VaultShareRole::Read => "READ",
            crate::models::VaultShareRole::Write => "WRITE",
            crate::models::VaultShareRole::Admin => "ADMIN",
        };
        role_badge.set_text(badge_text);
        role_badge.add_css_class("vault-share-role-badge");
        role_badge.set_margin_end(6);
        content.append(&role_badge);
    }

    let count_text = secret_count.to_string();
    let count_badge = gtk4::Label::new(Some(count_text.as_str()));
    count_badge.add_css_class("audit-count-badge");
    count_badge.set_margin_end(6);
    content.append(&count_badge);

    if can_delete {
        let delete_button = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .build();
        delete_button.add_css_class("flat");
        delete_button.set_valign(Align::Center);
        delete_button.set_tooltip_text(Some(crate::tr!("main-delete-vault-tooltip").as_str()));
        delete_button.set_opacity(0.0);
        delete_button.set_sensitive(false);
        delete_button.set_can_target(false);
        if let Some(delete_callback) = on_delete {
            let vault_name = title.to_string();
            delete_button.connect_clicked(move |_| {
                delete_callback(vault_id, vault_name.clone());
            });
        }

        let delete_button_enter = delete_button.clone();
        let delete_button_leave = delete_button.clone();
        let hover_controller = gtk4::EventControllerMotion::new();
        hover_controller.connect_enter(move |_controller, _x, _y| {
            delete_button_enter.set_opacity(1.0);
            delete_button_enter.set_sensitive(true);
            delete_button_enter.set_can_target(true);
        });
        hover_controller.connect_leave(move |_controller| {
            delete_button_leave.set_opacity(0.0);
            delete_button_leave.set_sensitive(false);
            delete_button_leave.set_can_target(false);
        });
        row.add_controller(hover_controller);

        content.append(&delete_button);
    }

    row.set_child(Some(&content));
    row.set_widget_name(format!("vault-{}", vault_id).as_str());
    row
}

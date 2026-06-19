use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use secrecy::{ExposeSecret, SecretBox};
use serde_json::Value;
use tokio::runtime::Handle;
use uuid::Uuid;

use crate::ui::dialogs::add_edit_dialog::DialogMode;
use crate::ui::messages;
use crate::ui::widgets::secret_card::{SecretCard, SecretRowData};
use crate::ui::windows::main_window::types::SecretQuickActions;
use heelonvault_core::models::SecretItem;
use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

use super::{search_filter, FilterRuntime, SecretFilterMeta, SecretKind, SecretRowView};

pub(super) fn evaluate_password_strength_label(secret_value: &str) -> String {
    if secret_value.len() >= 12 {
        let has_uppercase = secret_value.chars().any(|c| c.is_uppercase());
        let has_lowercase = secret_value.chars().any(|c| c.is_lowercase());
        let has_digit = secret_value.chars().any(|c| c.is_numeric());
        let has_special = secret_value.chars().any(|c| !c.is_alphanumeric());
        let complexity = [has_uppercase, has_lowercase, has_digit, has_special]
            .iter()
            .filter(|&&v| v)
            .count();
        if complexity >= 3 {
            return heelonvault_core::tr!("main-strength-strong");
        }
    }
    heelonvault_core::tr!("main-strength-weak")
}

/// Decode a single `SecretItem` into a `SecretRowView`.
/// Returns `None` if the secret cannot be opened (wrong key, corrupt, etc.).
async fn build_secret_row<TSecret>(
    item: SecretItem,
    secret_service: &Arc<TSecret>,
    vault_key: &secrecy::SecretBox<Vec<u8>>,
    vault_name: String,
    vault_access: (bool, bool, bool),
) -> Option<SecretRowView>
where
    TSecret: SecretService + Send + Sync + 'static,
{
    let secret_result = secret_service
        .get_secret(
            item.id,
            SecretBox::new(Box::new(vault_key.expose_secret().clone())),
        )
        .await;
    let secret_value = match secret_result {
        Ok(secret) => {
            String::from_utf8(secret.secret_value.expose_secret().clone()).unwrap_or_default()
        }
        Err(_) => String::new(),
    };

    let (login, email, url, notes, category, has_health_access_marker) =
        match item.metadata_json.as_deref() {
            Some(raw) => match serde_json::from_str::<Value>(raw) {
                Ok(value) => {
                    let login = value
                        .get("login")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let email = value
                        .get("email")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let url = value
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let notes = value
                        .get("notes")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let category = value
                        .get("category")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let has_health_access_marker = value
                        .get("health_access")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    (login, email, url, notes, category, has_health_access_marker)
                }
                Err(_) => (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    false,
                ),
            },
            None => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                false,
            ),
        };

    let (icon_name, type_label_text) = match item.secret_type {
        heelonvault_core::models::SecretType::Password => (
            "dialog-password-symbolic",
            heelonvault_core::tr!("secret-type-password"),
        ),
        heelonvault_core::models::SecretType::ApiToken => (
            "dialog-key-symbolic",
            heelonvault_core::tr!("secret-type-api-token"),
        ),
        heelonvault_core::models::SecretType::SshKey => (
            "network-wired-symbolic",
            heelonvault_core::tr!("secret-type-ssh-key"),
        ),
        heelonvault_core::models::SecretType::SecureDocument => (
            "folder-documents-symbolic",
            heelonvault_core::tr!("secret-type-secure-document"),
        ),
    };
    let (color_class, kind) = match item.secret_type {
        heelonvault_core::models::SecretType::Password => {
            ("secret-type-password", SecretKind::Password)
        }
        heelonvault_core::models::SecretType::ApiToken => {
            ("secret-type-token", SecretKind::ApiToken)
        }
        heelonvault_core::models::SecretType::SshKey => ("secret-type-ssh", SecretKind::SshKey),
        heelonvault_core::models::SecretType::SecureDocument => {
            ("secret-type-document", SecretKind::SecureDocument)
        }
    };

    let title = item.title.unwrap_or_else(|| type_label_text.clone());
    let created_at = item
        .created_at
        .unwrap_or_else(|| heelonvault_core::tr!("login-history-unavailable"));
    let health = evaluate_password_strength_label(secret_value.as_str());
    let tags = item.tags.clone().unwrap_or_default();
    let is_health_access = has_health_access_marker
        || search_filter::classify_health_access(
            title.as_str(),
            login.as_str(),
            url.as_str(),
            notes.as_str(),
            category.as_str(),
            tags.as_str(),
            type_label_text.as_str(),
        );

    Some(SecretRowView {
        secret_id: item.id,
        icon_name: icon_name.to_string(),
        type_label: type_label_text.to_string(),
        title,
        created_at,
        login,
        email,
        url,
        notes,
        category,
        tags,
        secret_value,
        kind,
        color_class: color_class.to_string(),
        health,
        is_health_access,
        usage_count: item.usage_count,
        vault_name,
        vault_access,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_secret_flow<TSecret, TVault>(
    _application: adw::Application,
    parent_window: adw::ApplicationWindow,
    runtime_handle: Handle,
    secret_service: Arc<TSecret>,
    vault_service: Arc<TVault>,
    admin_user_id: Uuid,
    admin_master_key: Vec<u8>,
    secret_flow: gtk4::FlowBox,
    stack: gtk4::Stack,
    empty_title: gtk4::Label,
    empty_copy: gtk4::Label,
    active_vault_id: Rc<RefCell<Option<Uuid>>>,
    toast_overlay: adw::ToastOverlay,
    filter_runtime: FilterRuntime,
    editor_launcher: Rc<RefCell<Option<Rc<dyn Fn(DialogMode)>>>>,
    search_all_vaults: bool,
) where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    empty_title.set_text(heelonvault_core::tr!("main-secrets-loading-title").as_str());
    empty_copy.set_text(heelonvault_core::tr!("main-secrets-loading-description").as_str());
    stack.set_visible_child_name("empty");

    let runtime_for_loader = runtime_handle.clone();
    let secret_for_loader = Arc::clone(&secret_service);
    let vault_for_loader = Arc::clone(&vault_service);
    let admin_master_for_loader = admin_master_key.clone();
    let selected_vault_id = *active_vault_id.borrow();

    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result: Result<
            (
                Option<(Uuid, bool, bool, bool)>,
                Vec<SecretRowView>,
                bool,
                bool,
            ),
            heelonvault_core::errors::AppError,
        > = runtime_for_loader.block_on(async move {
            if search_all_vaults {
                // ── Global cross-vault search ─────────────────────────────────
                let vaults = vault_for_loader.list_user_vaults(admin_user_id).await?;
                let mut all_rows: Vec<SecretRowView> = Vec::new();
                for vault in vaults {
                    let access = match vault_for_loader
                        .get_vault_access_for_user(admin_user_id, vault.id)
                        .await?
                    {
                        Some(a) => a,
                        None => continue,
                    };
                    let vault_key = match vault_for_loader
                        .open_vault_for_user(
                            admin_user_id,
                            vault.id,
                            SecretBox::new(Box::new(admin_master_for_loader.clone())),
                        )
                        .await
                    {
                        Ok(k) => k,
                        Err(_) => continue,
                    };
                    let items = match secret_for_loader.list_by_vault(vault.id).await {
                        Ok(i) => i,
                        Err(_) => continue,
                    };
                    let is_shared = vault.owner_user_id != admin_user_id;
                    let can_write = access.role.can_write();
                    let can_admin = access.role.can_admin();
                    for item in items {
                        let row = build_secret_row(
                            item,
                            &secret_for_loader,
                            &vault_key,
                            vault.name.clone(),
                            (is_shared, can_write, can_admin),
                        )
                        .await;
                        if let Some(r) = row {
                            all_rows.push(r);
                        }
                    }
                }
                Ok((None, all_rows, false, true))
            } else {
                // ── Single-vault mode (normal) ────────────────────────────────
                let vaults = vault_for_loader.list_user_vaults(admin_user_id).await?;
                let resolved_selected_id =
                    selected_vault_id.or_else(|| vaults.first().map(|vault| vault.id));
                let Some(selected_id) = resolved_selected_id else {
                    return Ok((None, Vec::new(), true, false));
                };

                let selected_vault = match vaults.into_iter().find(|vault| vault.id == selected_id)
                {
                    Some(value) => value,
                    None => return Ok((None, Vec::new(), false, false)),
                };
                let access = vault_for_loader
                    .get_vault_access_for_user(admin_user_id, selected_vault.id)
                    .await?
                    .ok_or({
                        heelonvault_core::errors::AppError::Authorization(
                            heelonvault_core::errors::AccessDeniedReason::VaultAccessDenied,
                        )
                    })?;
                let is_shared = selected_vault.owner_user_id != admin_user_id;
                let can_write = access.role.can_write();
                let can_admin = access.role.can_admin();
                let vault_state = Some((selected_vault.id, is_shared, can_write, can_admin));

                let vault_key = vault_for_loader
                    .open_vault_for_user(
                        admin_user_id,
                        selected_vault.id,
                        SecretBox::new(Box::new(admin_master_for_loader.clone())),
                    )
                    .await?;

                let items = secret_for_loader.list_by_vault(selected_vault.id).await?;
                let mut rows = Vec::with_capacity(items.len());
                for item in items {
                    if let Some(r) = build_secret_row(
                        item,
                        &secret_for_loader,
                        &vault_key,
                        String::new(),
                        (is_shared, can_write, can_admin),
                    )
                    .await
                    {
                        rows.push(r);
                    }
                }
                Ok((vault_state, rows, false, false))
            }
        });
        let _ = sender.send(result);
    });

    let active_vault_for_receiver = Rc::clone(&active_vault_id);
    glib::MainContext::default().spawn_local(async move {
        match receiver.await {
            Ok(Ok((vault_state, items, no_selection, is_global))) => {
                if let Some((vault_id, _, _, _)) = vault_state {
                    *active_vault_for_receiver.borrow_mut() = Some(vault_id);
                }

                if no_selection {
                    empty_title.set_text("Aucun coffre sélectionné");
                    empty_copy.set_text(
                        "Sélectionnez un coffre dans la barre latérale pour afficher ses secrets.",
                    );
                    stack.set_visible_child_name("empty");
                    return;
                }

                // In global search mode vault_state is intentionally None (cross-vault)
                // — only treat a None vault_state as an error in single-vault mode.
                if vault_state.is_none() && !is_global {
                    *active_vault_for_receiver.borrow_mut() = None;
                    empty_title.set_text("Coffre non disponible");
                    empty_copy.set_text(
                        "Le coffre sélectionné n'est plus accessible. Sélectionnez-en un autre.",
                    );
                    stack.set_visible_child_name("empty");
                    return;
                }

                filter_runtime.meta_by_widget.borrow_mut().clear();
                filter_runtime.actions_by_widget.borrow_mut().clear();
                filter_runtime.audit_all_count_label.set_text("0");
                filter_runtime.audit_weak_count_label.set_text("0");
                filter_runtime.audit_duplicate_count_label.set_text("0");
                filter_runtime.total_count_label.set_text("0");
                filter_runtime.non_compliant_count_label.set_text("0");
                filter_runtime.filtered_status_page.set_visible(false);

                while let Some(child) = secret_flow.first_child() {
                    secret_flow.remove(&child);
                }

                if items.is_empty() {
                    empty_title.set_text(heelonvault_core::tr!("main-empty-title").as_str());
                    empty_copy.set_text(heelonvault_core::tr!("main-empty-description").as_str());
                    stack.set_visible_child_name("empty");
                    return;
                }

                // Phase 3: sort in data-preparation stage (not during widget rendering)
                // so frequent secrets remain first even for large lists.
                let mut items = items;
                items.sort_by(|left, right| {
                    right
                        .usage_count
                        .cmp(&left.usage_count)
                        .then_with(|| left.title.cmp(&right.title))
                });

                let mut duplicate_counts: HashMap<String, usize> = HashMap::new();
                for item in &items {
                    if !item.secret_value.is_empty() {
                        *duplicate_counts
                            .entry(item.secret_value.clone())
                            .or_insert(0) += 1;
                    }
                }

                let shared_vault = vault_state
                    .map(|(_, is_shared, _, _)| is_shared)
                    .unwrap_or(false);
                let can_write = vault_state
                    .map(|(_, _, can_write, _)| can_write)
                    .unwrap_or(false);
                let can_admin = vault_state
                    .map(|(_, _, _, can_admin)| can_admin)
                    .unwrap_or(false);
                for (original_rank, item) in items.into_iter().enumerate() {
                    let is_duplicate = duplicate_counts
                        .get(&item.secret_value)
                        .copied()
                        .unwrap_or(0)
                        > 1;

                    // In multi-vault search mode the vault_state is None, so use
                    // the per-item vault_access tuple instead.
                    let (item_shared, item_can_write, item_can_admin) = if vault_state.is_some() {
                        (shared_vault, can_write, can_admin)
                    } else {
                        item.vault_access
                    };

                    let card_data = SecretRowData {
                        secret_id: item.secret_id,
                        icon_name: item.icon_name.clone(),
                        type_label: item.type_label.clone(),
                        title: item.title.clone(),
                        created_at: item.created_at.clone(),
                        login: item.login.clone(),
                        url: item.url.clone(),
                        secret_value: item.secret_value.clone(),
                        color_class: item.color_class.clone(),
                        health: item.health.clone(),
                        is_health_access: item.is_health_access,
                        usage_count: item.usage_count,
                        is_duplicate,
                        is_incomplete: item.login.trim().is_empty() || item.url.trim().is_empty(),
                        is_shared_vault: item_shared,
                        can_edit: !item_shared || item_can_write,
                        can_delete: !item_shared || item_can_admin,
                        vault_name: item.vault_name.clone(),
                    };

                    let card = Rc::new(SecretCard::new(card_data));
                    let copy_button = card.get_copy_button();
                    let copy_login_button = card.get_copy_login_button();
                    let open_url_button = card.get_open_url_button();
                    let usage_count = Rc::new(Cell::new(item.usage_count));
                    let kind = item.kind;

                    // ── Copy password (🔑) ─────────────────────────────────────────────────────
                    // copy_button is already desensitised in SecretCard::new when secret_value is
                    // empty; we only wire the handler when there is a value to copy.
                    if !item.secret_value.is_empty() {
                        let copy_value = item.secret_value.clone();
                        let copy_title_for_audit = item.title.clone();
                        let card_for_copy = Rc::clone(&card);
                        let service_for_copy = Arc::clone(&secret_service);
                        let runtime_for_copy = runtime_handle.clone();
                        let usage_for_copy = Rc::clone(&usage_count);
                        let secret_id_for_copy = item.secret_id;
                        let toast_overlay_for_copy = toast_overlay.clone();
                        copy_button.connect_clicked(move |_| {
                            if let Some(display) = gtk4::gdk::Display::default() {
                                display.clipboard().set_text(&copy_value);
                            }
                            toast_overlay_for_copy.add_toast(adw::Toast::new(
                                messages::toast_password_copied().as_str(),
                            ));

                            let new_value = usage_for_copy.get().saturating_add(1);
                            usage_for_copy.set(new_value);
                            card_for_copy.update_usage_count(new_value);

                            let service_for_task = Arc::clone(&service_for_copy);
                            let runtime_for_task = runtime_for_copy.clone();
                            std::thread::spawn(move || {
                                let _ = runtime_for_task.block_on(async move {
                                    service_for_task
                                        .increment_usage_count(secret_id_for_copy)
                                        .await
                                });
                            });

                            // ── CNIL: log field copy ──────────────────────
                            let service_for_audit = Arc::clone(&service_for_copy);
                            let runtime_for_audit = runtime_for_copy.clone();
                            let title_for_audit = copy_title_for_audit.clone();
                            std::thread::spawn(move || {
                                let _ = runtime_for_audit.block_on(async move {
                                    service_for_audit
                                        .record_field_copy(
                                            secret_id_for_copy,
                                            Some(admin_user_id),
                                            Some(title_for_audit.as_str()),
                                            "password",
                                        )
                                        .await
                                });
                            });
                        });
                    }

                    // ── Copy login (👤) ────────────────────────────────────────────────────────
                    // Button is only present when login is non-empty (see SecretCard::new).
                    if let Some(copy_login_btn) = copy_login_button.clone() {
                        let login_value = item.login.clone();
                        let login_title_for_audit = item.title.clone();
                        let card_for_login = Rc::clone(&card);
                        let service_for_login = Arc::clone(&secret_service);
                        let runtime_for_login = runtime_handle.clone();
                        let usage_for_login = Rc::clone(&usage_count);
                        let secret_id_for_login = item.secret_id;
                        let toast_overlay_for_login = toast_overlay.clone();
                        copy_login_btn.connect_clicked(move |_| {
                            if let Some(display) = gtk4::gdk::Display::default() {
                                display.clipboard().set_text(&login_value);
                            }
                            toast_overlay_for_login.add_toast(adw::Toast::new(
                                messages::toast_login_copied().as_str(),
                            ));

                            let new_value = usage_for_login.get().saturating_add(1);
                            usage_for_login.set(new_value);
                            card_for_login.update_usage_count(new_value);

                            let service_for_task = Arc::clone(&service_for_login);
                            let runtime_for_task = runtime_for_login.clone();
                            std::thread::spawn(move || {
                                let _ = runtime_for_task.block_on(async move {
                                    service_for_task
                                        .increment_usage_count(secret_id_for_login)
                                        .await
                                });
                            });

                            // CNIL: log login copy.
                            let service_for_audit = Arc::clone(&service_for_login);
                            let runtime_for_audit = runtime_for_login.clone();
                            let title_for_audit = login_title_for_audit.clone();
                            std::thread::spawn(move || {
                                let _ = runtime_for_audit.block_on(async move {
                                    service_for_audit
                                        .record_field_copy(
                                            secret_id_for_login,
                                            Some(admin_user_id),
                                            Some(title_for_audit.as_str()),
                                            "login",
                                        )
                                        .await
                                });
                            });
                        });
                    }

                    // ── Open URL (🌐) ──────────────────────────────────────────────────────────
                    // Button is only present when url is non-empty (see SecretCard::new).
                    if let Some(open_url_btn) = open_url_button.clone() {
                        let url_value = item.url.clone();
                        let login_value_for_url = item.login.clone();
                        let url_title_for_audit = item.title.clone();
                        let card_for_url = Rc::clone(&card);
                        let service_for_url = Arc::clone(&secret_service);
                        let runtime_for_url = runtime_handle.clone();
                        let usage_for_url = Rc::clone(&usage_count);
                        let secret_id_for_url = item.secret_id;
                        let toast_overlay_for_url = toast_overlay.clone();
                        let parent_window_for_url = parent_window.clone();
                        open_url_btn.connect_clicked(move |_| {
                            let copied_login = if !login_value_for_url.trim().is_empty() {
                                if let Some(display) = gtk4::gdk::Display::default() {
                                    display.clipboard().set_text(&login_value_for_url);
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            gtk4::show_uri(
                                Some(&parent_window_for_url),
                                &url_value,
                                gtk4::gdk::CURRENT_TIME,
                            );
                            let toast_message = if copied_login {
                                messages::toast_url_opened_login_copied()
                            } else {
                                messages::toast_url_opened()
                            };
                            toast_overlay_for_url
                                .add_toast(adw::Toast::new(toast_message.as_str()));

                            let new_value = usage_for_url.get().saturating_add(1);
                            usage_for_url.set(new_value);
                            card_for_url.update_usage_count(new_value);

                            let service_for_task = Arc::clone(&service_for_url);
                            let runtime_for_task = runtime_for_url.clone();
                            std::thread::spawn(move || {
                                let _ = runtime_for_task.block_on(async move {
                                    service_for_task
                                        .increment_usage_count(secret_id_for_url)
                                        .await
                                });
                            });

                            // CNIL: log URL open.
                            let service_for_audit = Arc::clone(&service_for_url);
                            let runtime_for_audit = runtime_for_url.clone();
                            let title_for_audit = url_title_for_audit.clone();
                            std::thread::spawn(move || {
                                runtime_for_audit.block_on(async move {
                                    let _ = service_for_audit
                                        .record_field_copy(
                                            secret_id_for_url,
                                            Some(admin_user_id),
                                            Some(title_for_audit.as_str()),
                                            "url_open",
                                        )
                                        .await;

                                    if copied_login {
                                        let _ = service_for_audit
                                            .record_field_copy(
                                                secret_id_for_url,
                                                Some(admin_user_id),
                                                Some(title_for_audit.as_str()),
                                                "login",
                                            )
                                            .await;
                                    }
                                });
                            });
                        });
                    }

                    let card_widget = card.get_widget();
                    let card_widget_for_hover = card_widget.clone().upcast::<gtk4::Widget>();
                    let flow_for_hover = secret_flow.clone();
                    let hover_controller = gtk4::EventControllerMotion::new();
                    hover_controller.connect_enter(move |_controller, _x, _y| {
                        if let Some(parent) = card_widget_for_hover.parent() {
                            if let Ok(flow_child) = parent.downcast::<gtk4::FlowBoxChild>() {
                                flow_for_hover.select_child(&flow_child);
                                flow_child.grab_focus();
                            }
                        }
                    });
                    card_widget.add_controller(hover_controller);

                    // Keep keyboard shortcuts aligned with the last card targeted by mouse.
                    let card_widget_for_select = card_widget.clone().upcast::<gtk4::Widget>();
                    let flow_for_select = secret_flow.clone();
                    let select_click = gtk4::GestureClick::new();
                    select_click.set_button(0);
                    select_click.connect_pressed(move |_, _, _, _| {
                        if let Some(parent) = card_widget_for_select.parent() {
                            if let Ok(flow_child) = parent.downcast::<gtk4::FlowBoxChild>() {
                                flow_for_select.select_child(&flow_child);
                                flow_child.grab_focus();
                            }
                        }
                    });
                    card_widget.add_controller(select_click);

                    // Open editor when clicking the card (outside quick-action buttons).
                    if !item_shared || item_can_write {
                        let editor_launcher_for_card = editor_launcher.clone();
                        let secret_id_for_card = item.secret_id;
                        let card_widget_for_pick = card_widget.clone().upcast::<gtk4::Widget>();
                        let card_click = gtk4::GestureClick::new();
                        card_click.set_button(0);
                        card_click.connect_released(move |_, n_press, x, y| {
                            if n_press < 2 {
                                return;
                            }

                            if let Some(picked) =
                                card_widget_for_pick.pick(x, y, gtk4::PickFlags::DEFAULT)
                            {
                                let mut current = Some(picked);
                                while let Some(widget) = current {
                                    if widget.has_css_class("secret-card-action-btn") {
                                        return;
                                    }
                                    current = widget.parent();
                                }
                            }

                            if let Some(open_editor) = editor_launcher_for_card.borrow().as_ref() {
                                open_editor(DialogMode::Edit(secret_id_for_card));
                            }
                        });
                        card_widget.add_controller(card_click);
                    }

                    let widget_key = format!("secret-card-{}", item.secret_id);
                    card_widget.set_widget_name(&widget_key);
                    filter_runtime.actions_by_widget.borrow_mut().insert(
                        widget_key.clone(),
                        SecretQuickActions {
                            copy_password: copy_button.clone(),
                            copy_login: copy_login_button.clone(),
                            open_url: open_url_button.clone(),
                        },
                    );
                    filter_runtime.meta_by_widget.borrow_mut().insert(
                        widget_key,
                        SecretFilterMeta {
                            searchable_text: search_filter::normalize_search_text(
                                [
                                    item.title.clone(),
                                    item.type_label.clone(),
                                    item.login.clone(),
                                    item.email.clone(),
                                    item.url.clone(),
                                    item.notes.clone(),
                                    item.category.clone(),
                                    item.tags.clone(),
                                    item.created_at.clone(),
                                    item.health.clone(),
                                    item.vault_name.clone(),
                                ]
                                .join(" ")
                                .as_str(),
                            ),
                            title_text: search_filter::normalize_search_text(item.title.as_str()),
                            login_text: search_filter::normalize_search_text(item.login.as_str()),
                            email_text: search_filter::normalize_search_text(item.email.as_str()),
                            url_text: search_filter::normalize_search_text(item.url.as_str()),
                            notes_text: search_filter::normalize_search_text(item.notes.as_str()),
                            category_text: search_filter::normalize_search_text(
                                item.category.as_str(),
                            ),
                            tags_text: search_filter::normalize_search_text(item.tags.as_str()),
                            type_text: search_filter::normalize_search_text(
                                [
                                    item.type_label.clone(),
                                    match kind {
                                        SecretKind::Password => {
                                            "password motdepasse mdp".to_string()
                                        }
                                        SecretKind::ApiToken => "token api acces".to_string(),
                                        SecretKind::SshKey => "ssh cle key".to_string(),
                                        SecretKind::SecureDocument => {
                                            "document fichier".to_string()
                                        }
                                    },
                                ]
                                .join(" ")
                                .as_str(),
                            ),
                            vault_name_text: search_filter::normalize_search_text(
                                item.vault_name.as_str(),
                            ),
                            kind,
                            original_rank,
                            is_weak: item.health == heelonvault_core::tr!("main-strength-weak"),
                            is_duplicate,
                            is_health: item.is_health_access,
                        },
                    );
                    secret_flow.insert(&card_widget, -1);
                }

                search_filter::apply_filters(&secret_flow, &filter_runtime);
                stack.set_visible_child_name("list");
            }
            Ok(Err(_)) | Err(_) => {
                empty_title.set_text(heelonvault_core::tr!("main-list-unavailable-title").as_str());
                empty_copy
                    .set_text(heelonvault_core::tr!("main-list-unavailable-description").as_str());
                stack.set_visible_child_name("empty");
            }
        }
    });
}

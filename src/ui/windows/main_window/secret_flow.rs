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

use crate::services::secret_service::SecretService;
use crate::services::vault_service::VaultService;
use crate::ui::dialogs::add_edit_dialog::DialogMode;
use crate::ui::messages;
use crate::ui::widgets::secret_card::{SecretCard, SecretRowData};

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
            return crate::tr!("main-strength-strong");
        }
    }
    crate::tr!("main-strength-weak")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_secret_flow<TSecret, TVault>(
    application: adw::Application,
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
) where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    empty_title.set_text(crate::tr!("main-secrets-loading-title").as_str());
    empty_copy.set_text(crate::tr!("main-secrets-loading-description").as_str());
    stack.set_visible_child_name("empty");

    let runtime_for_loader = runtime_handle.clone();
    let secret_for_loader = Arc::clone(&secret_service);
    let vault_for_loader = Arc::clone(&vault_service);
    let admin_master_for_loader = admin_master_key.clone();
    let selected_vault_id = *active_vault_id.borrow();

    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result: Result<
            (Option<(Uuid, bool, bool, bool)>, Vec<SecretRowView>, bool),
            crate::errors::AppError,
        > = runtime_for_loader.block_on(async move {
            let vaults = vault_for_loader.list_user_vaults(admin_user_id).await?;
            let resolved_selected_id =
                selected_vault_id.or_else(|| vaults.first().map(|vault| vault.id));
            let Some(selected_id) = resolved_selected_id else {
                return Ok((None, Vec::new(), true));
            };

            let selected_vault = match vaults.into_iter().find(|vault| vault.id == selected_id) {
                Some(value) => value,
                None => return Ok((None, Vec::new(), false)),
            };
            let access = vault_for_loader
                .get_vault_access_for_user(admin_user_id, selected_vault.id)
                .await?
                .ok_or({
                    crate::errors::AppError::Authorization(
                        crate::errors::AccessDeniedReason::VaultAccessDenied,
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
                let secret_result = secret_for_loader
                    .get_secret(
                        item.id,
                        SecretBox::new(Box::new(vault_key.expose_secret().clone())),
                    )
                    .await;
                let secret_value = match secret_result {
                    Ok(secret) => String::from_utf8(secret.secret_value.expose_secret().clone())
                        .unwrap_or_default(),
                    Err(_) => String::new(),
                };

                let (login, email, url, notes, category) = match item.metadata_json.as_deref() {
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
                            (login, email, url, notes, category)
                        }
                        Err(_) => (
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                        ),
                    },
                    None => (
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ),
                };

                let (icon_name, type_label_text) = match item.secret_type {
                    crate::models::SecretType::Password => (
                        "dialog-password-symbolic",
                        crate::tr!("secret-type-password"),
                    ),
                    crate::models::SecretType::ApiToken => {
                        ("dialog-key-symbolic", crate::tr!("secret-type-api-token"))
                    }
                    crate::models::SecretType::SshKey => {
                        ("network-wired-symbolic", crate::tr!("secret-type-ssh-key"))
                    }
                    crate::models::SecretType::SecureDocument => (
                        "folder-documents-symbolic",
                        crate::tr!("secret-type-secure-document"),
                    ),
                };
                let (color_class, kind) = match item.secret_type {
                    crate::models::SecretType::Password => {
                        ("secret-type-password", SecretKind::Password)
                    }
                    crate::models::SecretType::ApiToken => {
                        ("secret-type-token", SecretKind::ApiToken)
                    }
                    crate::models::SecretType::SshKey => ("secret-type-ssh", SecretKind::SshKey),
                    crate::models::SecretType::SecureDocument => {
                        ("secret-type-document", SecretKind::SecureDocument)
                    }
                };

                let title = item.title.unwrap_or_else(|| type_label_text.clone());
                let created_at = item
                    .created_at
                    .unwrap_or_else(|| crate::tr!("login-history-unavailable"));
                let health = evaluate_password_strength_label(secret_value.as_str());
                let tags = item.tags.clone().unwrap_or_default();

                rows.push(SecretRowView {
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
                    usage_count: item.usage_count,
                });
            }

            Ok((vault_state, rows, false))
        });
        let _ = sender.send(result);
    });

    let active_vault_for_receiver = Rc::clone(&active_vault_id);
    glib::MainContext::default().spawn_local(async move {
        match receiver.await {
            Ok(Ok((vault_state, items, no_selection))) => {
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

                if vault_state.is_none() {
                    *active_vault_for_receiver.borrow_mut() = None;
                    empty_title.set_text("Coffre non disponible");
                    empty_copy.set_text(
                        "Le coffre sélectionné n'est plus accessible. Sélectionnez-en un autre.",
                    );
                    stack.set_visible_child_name("empty");
                    return;
                }

                filter_runtime.meta_by_widget.borrow_mut().clear();
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
                    empty_title.set_text(crate::tr!("main-empty-title").as_str());
                    empty_copy.set_text(crate::tr!("main-empty-description").as_str());
                    stack.set_visible_child_name("empty");
                    return;
                }

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
                        usage_count: item.usage_count,
                        is_duplicate,
                        is_shared_vault: shared_vault,
                        can_edit: !shared_vault || can_write,
                        can_delete: !shared_vault || can_admin,
                    };

                    let card = Rc::new(SecretCard::new(card_data));
                    let usage_count = Rc::new(Cell::new(item.usage_count));
                    let kind = item.kind;

                    let editor_launcher_for_edit = editor_launcher.clone();
                    let secret_id_for_edit = item.secret_id;
                    card.get_edit_button().connect_clicked(move |_| {
                        if let Some(open_editor) = editor_launcher_for_edit.borrow().as_ref() {
                            open_editor(DialogMode::Edit(secret_id_for_edit));
                        }
                    });

                    let app_for_delete = application.clone();
                    let parent_for_delete = parent_window.clone();
                    let runtime_for_delete = runtime_handle.clone();
                    let secret_for_delete = Arc::clone(&secret_service);
                    let vault_for_delete = Arc::clone(&vault_service);
                    let flow_for_delete = secret_flow.clone();
                    let stack_for_delete = stack.clone();
                    let empty_title_for_delete = empty_title.clone();
                    let empty_copy_for_delete = empty_copy.clone();
                    let master_for_delete = admin_master_key.clone();
                    let filter_for_delete = filter_runtime.clone();
                    let editor_launcher_for_delete = editor_launcher.clone();
                    let active_vault_for_delete = active_vault_id.clone();
                    let secret_id_for_delete = item.secret_id;
                    let secret_title_for_delete = item.title.clone();
                    let toast_overlay_for_delete = toast_overlay.clone();
                    card.get_trash_button().connect_clicked(move |_| {
                        let (sender, receiver) = tokio::sync::oneshot::channel();
                        let secret_service_for_task = Arc::clone(&secret_for_delete);
                        let runtime_for_task = runtime_for_delete.clone();
                        std::thread::spawn(move || {
                            let result = runtime_for_task.block_on(async move {
                                secret_service_for_task
                                    .soft_delete(secret_id_for_delete)
                                    .await
                            });
                            let _ = sender.send(result);
                        });

                        let app_for_refresh = app_for_delete.clone();
                        let parent_for_refresh = parent_for_delete.clone();
                        let runtime_for_refresh = runtime_for_delete.clone();
                        let secret_for_refresh = Arc::clone(&secret_for_delete);
                        let vault_for_refresh = Arc::clone(&vault_for_delete);
                        let flow_for_refresh = flow_for_delete.clone();
                        let stack_for_refresh = stack_for_delete.clone();
                        let empty_title_refresh = empty_title_for_delete.clone();
                        let empty_copy_refresh = empty_copy_for_delete.clone();
                        let master_for_refresh = master_for_delete.clone();
                        let filter_for_refresh = filter_for_delete.clone();
                        let editor_launcher_for_refresh = editor_launcher_for_delete.clone();
                        let secret_title_for_refresh = secret_title_for_delete.clone();
                        let toast_overlay_for_refresh = toast_overlay_for_delete.clone();
                        let active_vault_for_refresh = active_vault_for_delete.clone();
                        glib::MainContext::default().spawn_local(async move {
                            if matches!(receiver.await, Ok(Ok(()))) {
                                let toast_message = messages::toast_secret_deleted(
                                    secret_title_for_refresh.as_str(),
                                );
                                toast_overlay_for_refresh
                                    .add_toast(adw::Toast::new(toast_message.as_str()));
                                refresh_secret_flow(
                                    app_for_refresh.clone(),
                                    parent_for_refresh.clone(),
                                    runtime_for_refresh.clone(),
                                    Arc::clone(&secret_for_refresh),
                                    Arc::clone(&vault_for_refresh),
                                    admin_user_id,
                                    master_for_refresh.clone(),
                                    flow_for_refresh.clone(),
                                    stack_for_refresh.clone(),
                                    empty_title_refresh.clone(),
                                    empty_copy_refresh.clone(),
                                    active_vault_for_refresh.clone(),
                                    toast_overlay_for_refresh.clone(),
                                    filter_for_refresh.clone(),
                                    editor_launcher_for_refresh.clone(),
                                );
                            }
                        });
                    });

                    let copy_value = if !item.secret_value.is_empty() {
                        item.secret_value.clone()
                    } else {
                        item.login.clone()
                    };

                    if copy_value.is_empty() {
                        card.get_copy_button().set_sensitive(false);
                    } else {
                        let card_for_copy = Rc::clone(&card);
                        let service_for_copy = Arc::clone(&secret_service);
                        let runtime_for_copy = runtime_handle.clone();
                        let usage_for_copy = Rc::clone(&usage_count);
                        let secret_id_for_copy = item.secret_id;
                        card.get_copy_button().connect_clicked(move |_| {
                            if let Some(display) = gtk4::gdk::Display::default() {
                                display.clipboard().set_text(&copy_value);
                            }

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
                        });
                    }

                    let card_widget = card.get_widget();
                    let widget_key = format!("secret-card-{}", item.secret_id);
                    card_widget.set_widget_name(&widget_key);
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
                            kind,
                            original_rank,
                            is_weak: item.health == crate::tr!("main-strength-weak"),
                            is_duplicate,
                        },
                    );
                    secret_flow.insert(&card_widget, -1);
                }

                search_filter::apply_filters(&secret_flow, &filter_runtime);
                stack.set_visible_child_name("list");
            }
            Ok(Err(_)) | Err(_) => {
                empty_title.set_text(crate::tr!("main-list-unavailable-title").as_str());
                empty_copy.set_text(crate::tr!("main-list-unavailable-description").as_str());
                stack.set_visible_child_name("empty");
            }
        }
    });
}

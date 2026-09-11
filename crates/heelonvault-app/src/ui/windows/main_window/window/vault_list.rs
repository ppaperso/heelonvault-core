//! Vault sidebar: population, selection, creation and deletion.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;
use secrecy::SecretBox;
use tokio::runtime::Handle;
use uuid::Uuid;

use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

use crate::ui::windows::main_window::sidebar;
use crate::ui::windows::main_window::{MainWindow, SidebarWidgets};

/// Vault rows encode their identity in the widget name as `vault-<uuid>`.
pub fn vault_id_from_row(row: &gtk4::ListBoxRow) -> Option<Uuid> {
    row.widget_name()
        .strip_prefix("vault-")
        .and_then(|raw| Uuid::parse_str(raw).ok())
}

pub fn find_vault_row(list: &gtk4::ListBox, vault_id: Uuid) -> Option<gtk4::ListBoxRow> {
    let mut child_opt = list.first_child();
    while let Some(child) = child_opt {
        let next = child.next_sibling();
        if let Ok(row) = child.clone().downcast::<gtk4::ListBoxRow>()
            && vault_id_from_row(&row) == Some(vault_id)
        {
            return Some(row);
        }
        child_opt = next;
    }
    None
}

pub struct VaultListDeps<TSecret, TVault> {
    pub window: adw::ApplicationWindow,
    pub runtime_handle: Handle,
    pub secret_service: Arc<TSecret>,
    pub vault_service: Arc<TVault>,
    pub user_id: Uuid,
    pub session_master_key: Rc<RefCell<Vec<u8>>>,
    pub active_vault_id: Rc<RefCell<Option<Uuid>>>,
    pub my_vaults_list: gtk4::ListBox,
    pub shared_vaults_title: gtk4::Label,
    pub shared_vaults_list: gtk4::ListBox,
    /// Guards programmatic selection changes from re-entering the selection handlers.
    pub selection_sync: Rc<Cell<bool>>,
    /// Prevents concurrent auto-creation of the implicit first vault.
    pub default_vault_creation: Rc<Cell<bool>>,
    /// Reloads the secret list once the vault sections settle.
    pub refresh_secrets: Rc<dyn Fn()>,
    /// Filled with the full refresh so mutations (delete) can re-run it.
    pub refresh_after_mutation: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
}

/// Build the confirm-then-delete action handed to owned vault rows.
fn build_delete_vault_action<TVault>(
    window: adw::ApplicationWindow,
    runtime_handle: Handle,
    vault_service: Arc<TVault>,
    user_id: Uuid,
    active_vault_id: Rc<RefCell<Option<Uuid>>>,
    refresh_after_mutation: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) -> Rc<dyn Fn(Uuid, String)>
where
    TVault: VaultService + Send + Sync + 'static,
{
    Rc::new(move |vault_id: Uuid, vault_name: String| {
        let body = heelonvault_core::i18n::tr_args(
            "main-delete-vault-confirm-body",
            &[(
                "name",
                heelonvault_core::i18n::I18nArg::Str(vault_name.as_str()),
            )],
        );
        let dialog = adw::MessageDialog::new(
            Some(&window),
            Some(heelonvault_core::tr!("main-delete-vault-confirm-title").as_str()),
            Some(body.as_str()),
        );
        dialog.add_response("cancel", heelonvault_core::tr!("common-cancel").as_str());
        dialog.add_response(
            "delete",
            heelonvault_core::tr!("main-delete-vault-confirm-cta").as_str(),
        );
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let runtime_for_confirm = runtime_handle.clone();
        let vault_service_for_confirm = Arc::clone(&vault_service);
        let window_for_confirm = window.clone();
        let refresh_holder = Rc::clone(&refresh_after_mutation);
        let active_for_confirm = Rc::clone(&active_vault_id);
        dialog.connect_response(None, move |_dlg, response| {
            if response != "delete" {
                return;
            }

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let runtime_for_task = runtime_for_confirm.clone();
            let vault_service_for_task = Arc::clone(&vault_service_for_confirm);
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    vault_service_for_task.delete_vault(user_id, vault_id).await
                });
                let _ = sender.send(result);
            });

            let window_for_result = window_for_confirm.clone();
            let refresh_for_result = Rc::clone(&refresh_holder);
            let active_after_result = Rc::clone(&active_for_confirm);
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(())) => {
                        if *active_after_result.borrow() == Some(vault_id) {
                            *active_after_result.borrow_mut() = None;
                        }
                        if let Some(refresh) = refresh_for_result.borrow().as_ref() {
                            refresh();
                        }
                    }
                    Ok(Err(error)) => {
                        MainWindow::show_feedback_dialog(
                            &window_for_result,
                            heelonvault_core::tr!("main-delete-vault-error-title").as_str(),
                            error.to_string().as_str(),
                        );
                    }
                    Err(_) => {
                        MainWindow::show_feedback_dialog(
                            &window_for_result,
                            heelonvault_core::tr!("main-delete-vault-error-title").as_str(),
                            heelonvault_core::tr!("main-list-unavailable-description").as_str(),
                        );
                    }
                }
            });
        });
        dialog.present();
    })
}

/// Build the callback that reloads both vault lists and then the secret list.
///
/// When the user owns no vault yet, an implicit "perso" vault is created so the app is
/// never in a state where a secret cannot be stored anywhere.
pub fn build_refresh_vault_sections<TSecret, TVault>(
    deps: VaultListDeps<TSecret, TVault>,
) -> Rc<dyn Fn()>
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    Rc::new(move || {
        let delete_vault_action = build_delete_vault_action(
            deps.window.clone(),
            deps.runtime_handle.clone(),
            Arc::clone(&deps.vault_service),
            deps.user_id,
            Rc::clone(&deps.active_vault_id),
            Rc::clone(&deps.refresh_after_mutation),
        );

        while let Some(child) = deps.my_vaults_list.first_child() {
            deps.my_vaults_list.remove(&child);
        }
        while let Some(child) = deps.shared_vaults_list.first_child() {
            deps.shared_vaults_list.remove(&child);
        }

        let can_attempt_default_create = !deps.default_vault_creation.get();
        let master_for_default_create = if can_attempt_default_create {
            MainWindow::snapshot_session_master_key(&deps.session_master_key)
        } else {
            None
        };
        if can_attempt_default_create && master_for_default_create.is_some() {
            deps.default_vault_creation.set(true);
        }

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = deps.runtime_handle.clone();
        let vault_service_for_task = Arc::clone(&deps.vault_service);
        let secret_service_for_task = Arc::clone(&deps.secret_service);
        let user_id = deps.user_id;
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                let mut owned_vaults = vault_service_for_task.list_owned_vaults(user_id).await?;
                if owned_vaults.is_empty()
                    && let Some(master_key) = master_for_default_create
                {
                    let _ = vault_service_for_task
                        .create_vault(user_id, "perso", SecretBox::new(Box::new(master_key)))
                        .await;
                    owned_vaults = vault_service_for_task.list_owned_vaults(user_id).await?;
                }

                let mut owned_with_counts: Vec<(heelonvault_core::models::Vault, usize, bool)> =
                    Vec::with_capacity(owned_vaults.len());
                for vault in owned_vaults {
                    let count = secret_service_for_task.list_by_vault(vault.id).await?.len();
                    let is_shared_with_others = vault_service_for_task
                        .is_vault_shared_with_others(user_id, vault.id)
                        .await?;
                    owned_with_counts.push((vault, count, is_shared_with_others));
                }

                let shared_access = vault_service_for_task
                    .list_shared_vault_access(user_id)
                    .await?;
                let mut shared_with_counts: Vec<(
                    heelonvault_core::models::AccessibleVault,
                    usize,
                )> = Vec::with_capacity(shared_access.len());
                for access in shared_access {
                    let count = secret_service_for_task
                        .list_by_vault(access.vault.id)
                        .await?
                        .len();
                    shared_with_counts.push((access, count));
                }

                Ok::<_, heelonvault_core::errors::AppError>((owned_with_counts, shared_with_counts))
            });
            let _ = sender.send((result, can_attempt_default_create));
        });

        let my_vaults_list = deps.my_vaults_list.clone();
        let shared_vaults_title = deps.shared_vaults_title.clone();
        let shared_vaults_list = deps.shared_vaults_list.clone();
        let active_vault_id = Rc::clone(&deps.active_vault_id);
        let selection_sync = Rc::clone(&deps.selection_sync);
        let default_vault_creation = Rc::clone(&deps.default_vault_creation);
        let refresh_secrets = Rc::clone(&deps.refresh_secrets);
        glib::MainContext::default().spawn_local(async move {
            let Ok((result, attempted_default_create)) = receiver.await else {
                selection_sync.set(false);
                return;
            };
            if attempted_default_create {
                default_vault_creation.set(false);
            }
            let Ok((owned_vaults, shared_vaults)) = result else {
                selection_sync.set(false);
                return;
            };

            selection_sync.set(true);
            let mut first_vault_id: Option<Uuid> = None;
            for (vault, secret_count, is_shared_with_others) in owned_vaults {
                if first_vault_id.is_none() {
                    first_vault_id = Some(vault.id);
                }
                let row = sidebar::build_vault_sidebar_row(
                    vault.name.as_str(),
                    vault.id,
                    true,
                    is_shared_with_others,
                    None,
                    secret_count,
                    Some(Rc::clone(&delete_vault_action)),
                );
                my_vaults_list.append(&row);
            }

            let mut first_shared_vault_id: Option<Uuid> = None;
            for (access, secret_count) in shared_vaults {
                if first_shared_vault_id.is_none() {
                    first_shared_vault_id = Some(access.vault.id);
                }
                let row = sidebar::build_vault_sidebar_row(
                    access.vault.name.as_str(),
                    access.vault.id,
                    false,
                    false,
                    Some(access.role),
                    secret_count,
                    None,
                );
                shared_vaults_list.append(&row);
            }

            let has_shared = shared_vaults_list.first_child().is_some();
            shared_vaults_title.set_visible(has_shared);
            shared_vaults_list.set_visible(has_shared);

            select_active_vault(
                &my_vaults_list,
                &shared_vaults_list,
                &active_vault_id,
                first_vault_id,
                first_shared_vault_id,
            );

            selection_sync.set(false);
            refresh_secrets();
        });
    })
}

/// Re-select the active vault after a reload, falling back to the first available one.
fn select_active_vault(
    my_vaults_list: &gtk4::ListBox,
    shared_vaults_list: &gtk4::ListBox,
    active_vault_id: &Rc<RefCell<Option<Uuid>>>,
    first_vault_id: Option<Uuid>,
    first_shared_vault_id: Option<Uuid>,
) {
    let active = *active_vault_id.borrow();
    if let Some(active_id) = active {
        if let Some(row) = find_vault_row(my_vaults_list, active_id) {
            my_vaults_list.select_row(Some(&row));
            return;
        }
        if let Some(row) = find_vault_row(shared_vaults_list, active_id) {
            shared_vaults_list.select_row(Some(&row));
            return;
        }
    }

    if let Some(fallback_id) = first_vault_id {
        *active_vault_id.borrow_mut() = Some(fallback_id);
        if let Some(row) = find_vault_row(my_vaults_list, fallback_id) {
            my_vaults_list.select_row(Some(&row));
        }
    } else if let Some(fallback_id) = first_shared_vault_id {
        *active_vault_id.borrow_mut() = Some(fallback_id);
        if let Some(row) = find_vault_row(shared_vaults_list, fallback_id) {
            shared_vaults_list.select_row(Some(&row));
        }
    } else {
        *active_vault_id.borrow_mut() = None;
        my_vaults_list.unselect_all();
        shared_vaults_list.unselect_all();
    }
}

/// Wire selection on both vault lists: selecting in one clears the other.
pub fn setup_vault_selection_handlers(
    sidebar_panel: &SidebarWidgets,
    main_stack: &gtk4::Stack,
    active_vault_id: Rc<RefCell<Option<Uuid>>>,
    selection_sync: Rc<Cell<bool>>,
    refresh_secrets: Rc<dyn Fn()>,
) {
    let shared_list_for_my = sidebar_panel.shared_vaults_list.clone();
    let active_for_my = Rc::clone(&active_vault_id);
    let sync_for_my = Rc::clone(&selection_sync);
    let refresh_for_my = Rc::clone(&refresh_secrets);
    let stack_for_my = main_stack.clone();
    sidebar_panel
        .my_vaults_list
        .connect_row_selected(move |_list, row_opt| {
            if sync_for_my.get() {
                return;
            }
            if let Some(row) = row_opt
                && let Some(vault_id) = vault_id_from_row(row)
            {
                sync_for_my.set(true);
                shared_list_for_my.unselect_all();
                sync_for_my.set(false);
                *active_for_my.borrow_mut() = Some(vault_id);
                stack_for_my.set_visible_child_name("entries_view");
                refresh_for_my();
            }
        });

    let my_list_for_shared = sidebar_panel.my_vaults_list.clone();
    let active_for_shared = Rc::clone(&active_vault_id);
    let sync_for_shared = Rc::clone(&selection_sync);
    let refresh_for_shared = Rc::clone(&refresh_secrets);
    let stack_for_shared = main_stack.clone();
    sidebar_panel
        .shared_vaults_list
        .connect_row_selected(move |_list, row_opt| {
            if sync_for_shared.get() {
                return;
            }
            if let Some(row) = row_opt
                && let Some(vault_id) = vault_id_from_row(row)
            {
                sync_for_shared.set(true);
                my_list_for_shared.unselect_all();
                sync_for_shared.set(false);
                *active_for_shared.borrow_mut() = Some(vault_id);
                stack_for_shared.set_visible_child_name("entries_view");
                refresh_for_shared();
            }
        });
}

/// Wire the "create vault" button to its modal name prompt.
pub fn setup_create_vault_button<TVault>(
    create_vault_button: &gtk4::Button,
    window: adw::ApplicationWindow,
    runtime_handle: Handle,
    vault_service: Arc<TVault>,
    user_id: Uuid,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    refresh_after_create: Rc<dyn Fn()>,
) where
    TVault: VaultService + Send + Sync + 'static,
{
    create_vault_button.connect_clicked(move |_| {
        let create_window = gtk4::Window::builder()
            .title(heelonvault_core::tr!("main-create-vault-window-title").as_str())
            .modal(true)
            .transient_for(&window)
            .default_width(440)
            .default_height(140)
            .build();

        let root = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let name_entry = gtk4::Entry::new();
        name_entry.set_placeholder_text(Some(
            heelonvault_core::tr!("main-create-vault-name-placeholder").as_str(),
        ));

        let actions = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .halign(Align::End)
            .build();
        let cancel = gtk4::Button::with_label(heelonvault_core::tr!("common-cancel").as_str());
        let create =
            gtk4::Button::with_label(heelonvault_core::tr!("main-create-vault-create").as_str());
        create.add_css_class("suggested-action");
        actions.append(&cancel);
        actions.append(&create);

        root.append(&name_entry);
        root.append(&actions);
        create_window.set_child(Some(&root));

        let create_window_for_cancel = create_window.clone();
        cancel.connect_clicked(move |_| create_window_for_cancel.close());

        let runtime_for_apply = runtime_handle.clone();
        let vault_for_apply = Arc::clone(&vault_service);
        let session_for_apply = Rc::clone(&session_master_key);
        let window_for_apply = window.clone();
        let refresh_after_apply = Rc::clone(&refresh_after_create);
        let create_window_for_apply = create_window.clone();
        create.connect_clicked(move |_| {
            let vault_name = name_entry.text().to_string();
            if vault_name.trim().is_empty() {
                MainWindow::show_feedback_dialog(
                    &window_for_apply,
                    heelonvault_core::tr!("main-create-vault-window-title").as_str(),
                    heelonvault_core::tr!("main-create-vault-error-empty-name").as_str(),
                );
                return;
            }

            let Some(master_key) = MainWindow::snapshot_session_master_key(&session_for_apply)
            else {
                MainWindow::show_feedback_dialog(
                    &window_for_apply,
                    heelonvault_core::tr!("main-create-vault-window-title").as_str(),
                    heelonvault_core::tr!("main-create-vault-error-session-locked").as_str(),
                );
                return;
            };

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let runtime_for_task = runtime_for_apply.clone();
            let vault_for_task = Arc::clone(&vault_for_apply);
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    vault_for_task
                        .create_vault(
                            user_id,
                            vault_name.trim(),
                            SecretBox::new(Box::new(master_key)),
                        )
                        .await
                });
                let _ = sender.send(result);
            });

            let window_for_result = window_for_apply.clone();
            let refresh_for_result = Rc::clone(&refresh_after_apply);
            let create_window_close = create_window_for_apply.clone();
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(_)) => {
                        create_window_close.close();
                        refresh_for_result();
                    }
                    Ok(Err(error)) => {
                        MainWindow::show_feedback_dialog(
                            &window_for_result,
                            heelonvault_core::tr!("main-create-vault-window-title").as_str(),
                            error.to_string().as_str(),
                        );
                    }
                    Err(_) => {
                        MainWindow::show_feedback_dialog(
                            &window_for_result,
                            heelonvault_core::tr!("main-create-vault-window-title").as_str(),
                            heelonvault_core::tr!("main-list-unavailable-description").as_str(),
                        );
                    }
                }
            });
        });

        create_window.present();
    });
}

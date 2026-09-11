//! Events module - Event handlers for main window
//!
//! This module contains all event handlers and callback setup functions.
//! Each handler function takes the necessary state and widgets as parameters.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use sqlx::SqlitePool;
use tokio::runtime::Handle;
use tracing::{info, warn};
use uuid::Uuid;

#[allow(unused_imports)]
use crate::ui::windows::main_window::{AuditFilter, SecretCategoryFilter, SecretSortMode};

/// Setup the window close request handler
/// 
/// This handler checks if there are critical operations in flight and defers
/// the close request if so, otherwise persists the window state and triggers logout.
pub fn setup_window_close_handler(
    window: &adw::ApplicationWindow,
    critical_ops_in_flight: Rc<Cell<u32>>,
    on_logout: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) {
    let critical_ops_for_close = Rc::clone(&critical_ops_in_flight);
    let on_logout_for_close = Rc::clone(&on_logout);
    
    window.connect_close_request(move |win| {
        info!("main window close requested");
        if critical_ops_for_close.get() > 0 {
            warn!(
                critical_ops_in_flight = critical_ops_for_close.get(),
                "main window close deferred because a critical operation is still running"
            );
            // TODO: This should use the translated strings from the original
            // For now, we'll keep the French text to maintain functionality
            crate::ui::windows::main_window::MainWindow::show_feedback_dialog(
                win,
                "Opération en cours",
                "Une opération d'import/export est en cours. Attendez la fin avant de fermer la fenêtre.",
            );
            return glib::Propagation::Stop;
        }

        crate::ui::window_sizing::persist_main_window_state(
            win.width(),
            win.height(),
            win.is_fullscreen(),
        );

        info!("main window state persisted, routing close request to logout flow");

        if let Some(callback) = on_logout_for_close.borrow().as_ref() {
            callback();
        }
        glib::Propagation::Stop
    });
}

/// Setup the profile popover show handler
/// 
/// Refreshes the login history when the popover is shown.
pub fn setup_profile_popover_handlers(
    profile_popover: &gtk4::Popover,
    profile_button: &gtk4::MenuButton,
    runtime_handle: Handle,
    database_pool: SqlitePool,
    admin_user_id: Uuid,
    login_history_list: gtk4::Box,
) {
    let runtime_for_login_history = runtime_handle.clone();
    let db_for_login_history = database_pool.clone();
    let history_list_for_show = login_history_list.clone();
    let runtime_for_login_history_click = runtime_handle.clone();
    let db_for_login_history_click = database_pool.clone();
    let history_list_for_click = login_history_list.clone();
    
    profile_popover.connect_show(move |_| {
        super::super::MainWindow::refresh_login_history_popover(
            runtime_for_login_history.clone(),
            db_for_login_history.clone(),
            admin_user_id,
            history_list_for_show.clone(),
        );
    });
    
    profile_button.connect_notify_local(Some("active"), move |button, _| {
        if !button.property::<bool>("active") {
            return;
        }
        super::super::MainWindow::refresh_login_history_popover(
            runtime_for_login_history_click.clone(),
            db_for_login_history_click.clone(),
            admin_user_id,
            history_list_for_click.clone(),
        );
    });
}

/// Refresh the login history in the profile popover
/// 
/// This is called when the popover is shown or when the profile button is clicked.
#[allow(unused_variables)]
#[allow(dead_code)]
#[allow(dead_code)]
pub fn refresh_login_history_popover(
    _runtime: Handle,
    _database_pool: SqlitePool,
    _user_id: Uuid,
    _login_history_list: gtk4::Box,
) {
    // TODO: Implement the actual login history refresh logic
    // This would involve calling list_recent_logins and adding entries to the list
}

/// Setup the PIN status badge clicked handler
/// 
/// Opens the PIN setup dialog when the badge is clicked.
#[allow(clippy::too_many_arguments)]
pub fn setup_pin_badge_handler(
    header_pin_btn: &gtk4::Button,
    window: adw::ApplicationWindow,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    pin_cache: Rc<RefCell<Option<heelonvault_core::services::pin_cache_service::PinCache>>>,
    on_pin_state_cb: Rc<RefCell<Option<Rc<dyn Fn(bool)>>>>,
    admin_user_id: Uuid,
) {
    let win_for_header_pin = window.clone();
    let session_for_header_pin = Rc::clone(&session_master_key);
    let pin_cache_for_header_pin = Rc::clone(&pin_cache);
    let on_pin_state_for_header = Rc::clone(&on_pin_state_cb);
    
    header_pin_btn.connect_clicked(move |_| {
        let key_snapshot = {
            let k = session_for_header_pin.borrow();
            if k.is_empty() {
                return;
            }
            secrecy::SecretBox::new(Box::new(k.clone()))
        };
        let currently_active = pin_cache_for_header_pin.borrow().is_some();
        let cache_for_created = Rc::clone(&pin_cache_for_header_pin);
        let state_for_created = Rc::clone(&on_pin_state_for_header);
        let cache_for_disabled = Rc::clone(&pin_cache_for_header_pin);
        let state_for_disabled = Rc::clone(&on_pin_state_for_header);
        
        let dialog = crate::ui::dialogs::pin_setup_dialog::PinSetupDialog::new(
            &win_for_header_pin,
            key_snapshot,
            admin_user_id,
            move |new_cache| {
                *cache_for_created.borrow_mut() = Some(new_cache);
                if let Some(ref cb) = *state_for_created.borrow() {
                    cb(true);
                }
            },
            move || {
                let _ = cache_for_disabled.borrow_mut().take();
                if let Some(ref cb) = *state_for_disabled.borrow() {
                    cb(false);
                }
            },
            currently_active,
        );
        dialog.present();
    });
}

/// Setup the panic button clicked handler
/// 
/// Shows a confirmation dialog and exits the application if confirmed.
pub fn setup_panic_button_handler(panic_button: &gtk4::Button, window: adw::ApplicationWindow) {
    let window_for_panic = window.clone();
    
    panic_button.connect_clicked(move |_| {
        let dialog = adw::MessageDialog::new(
            Some(&window_for_panic),
            Some(heelonvault_core::tr!("main-panic-title").as_str()),
            Some(heelonvault_core::tr!("main-panic-body").as_str()),
        );
        dialog.add_response("cancel", heelonvault_core::tr!("main-panic-cancel").as_str());
        dialog.add_response("wipe_exit", heelonvault_core::tr!("main-panic-confirm").as_str());
        dialog.set_response_appearance("wipe_exit", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.connect_response(None, |_dlg, response| {
            if response == "wipe_exit" {
                // Journalise l'événement avant la sortie.
                // DailyLogFileWriter est synchrone : le message est écrit
                // sur disque avant que process::exit ne termine le processus.
                info!("Panic mode activated - wiping memory and exiting");
                // Les SecretBox allouées dans les closures GTK seront
                // retirées de la mémoire virtuelle par le noyau lors de la
                // libération du tas du processus. L'OS zero-fill les pages
                // avant de les réattribuer (comportement garanti par Linux).
                std::process::exit(0);
            }
        });
        dialog.present();
    });
}

/// Setup the add button clicked handler
/// 
/// Opens the editor dialog for creating a new secret.
/// 
/// Note: This uses a generic type parameter for VaultService to avoid dyn compatibility issues.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn setup_add_button_handler<TVault>(
    add_button: &gtk4::Button,
    window: adw::ApplicationWindow,
    active_vault_id: Rc<RefCell<Option<Uuid>>>,
    runtime_handle: Handle,
    vault_service: Arc<TVault>,
    open_editor: Rc<dyn Fn(crate::ui::dialogs::add_edit_dialog::DialogMode)>,
    admin_user_id: Uuid,
) where
    TVault: heelonvault_core::services::vault_service::VaultService + Send + Sync + 'static,
{
    let open_editor_for_add = Rc::clone(&open_editor);
    let active_vault_for_add = Rc::clone(&active_vault_id);
    let runtime_for_add = runtime_handle.clone();
    let vault_service_for_add = Arc::clone(&vault_service);
    let window_for_add = window.clone();
    
    add_button.connect_clicked(move |_| {
        let maybe_vault_id = *active_vault_for_add.borrow();
        let Some(vault_id) = maybe_vault_id else {
            open_editor_for_add(crate::ui::dialogs::add_edit_dialog::DialogMode::Create);
            return;
        };

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_add.clone();
        let vault_for_task = Arc::clone(&vault_service_for_add);
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                let access = vault_for_task
                    .get_vault_access_for_user(admin_user_id, vault_id)
                    .await?
                    .ok_or({
                        heelonvault_core::errors::AppError::Authorization(
                            heelonvault_core::errors::AccessDeniedReason::VaultAccessDenied,
                        )
                    })?;
                let is_shared = access.vault.owner_user_id != admin_user_id;
                Ok::<bool, heelonvault_core::errors::AppError>(!is_shared || access.role.can_admin())
            });
            let _ = sender.send(result);
        });

        let open_editor_for_result = Rc::clone(&open_editor_for_add);
        let window_for_result = window_for_add.clone();
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(true)) => {
                    open_editor_for_result(crate::ui::dialogs::add_edit_dialog::DialogMode::CreateInVault(vault_id));
                }
                Ok(Ok(false)) => {
                    crate::ui::windows::main_window::MainWindow::show_feedback_dialog(
                        &window_for_result,
                        heelonvault_core::tr!("main-add-shared-denied-title").as_str(),
                        heelonvault_core::tr!("main-add-shared-denied-body").as_str(),
                    );
                }
                _ => {
                    crate::ui::windows::main_window::MainWindow::show_feedback_dialog(
                        &window_for_result,
                        heelonvault_core::tr!("main-add-shared-denied-title").as_str(),
                        heelonvault_core::tr!("main-list-unavailable-description").as_str(),
                    );
                }
            }
        });
    });
}

/// Setup the trash button clicked handler
/// 
/// Opens the trash dialog for managing deleted secrets.
/// 
/// Note: This uses generic type parameters to avoid dyn compatibility issues.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn setup_trash_button_handler<TSecret, TVault>(
    trash_button: &gtk4::Button,
    application: &adw::Application,
    window: adw::ApplicationWindow,
    runtime_handle: Handle,
    secret_service: Arc<TSecret>,
    vault_service: Arc<TVault>,
    admin_user_id: Uuid,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    refresh_after_trash: Rc<dyn Fn()>,
) where
    TSecret: heelonvault_core::services::secret_service::SecretService + Send + Sync + 'static,
    TVault: heelonvault_core::services::vault_service::VaultService + Send + Sync + 'static,
{
    let app_for_trash = application.clone();
    let window_for_trash = window.clone();
    let runtime_for_trash = runtime_handle.clone();
    let secret_for_trash = Arc::clone(&secret_service);
    let vault_for_trash = Arc::clone(&vault_service);
    let admin_user_for_trash = admin_user_id;
    let session_master_for_trash = Rc::clone(&session_master_key);
    let refresh_for_trash = Rc::clone(&refresh_after_trash);
    
    trash_button.connect_clicked(move |_| {
        let Some(master_key) = super::super::MainWindow::snapshot_session_master_key(&session_master_for_trash)
        else {
            info!("trash access blocked: session is locked");
            return;
        };
        let refresh_after_trash = Rc::clone(&refresh_for_trash);
        let dialog = crate::ui::dialogs::trash_dialog::TrashDialog::new(
            &app_for_trash,
            &window_for_trash,
            runtime_for_trash.clone(),
            Arc::clone(&secret_for_trash),
            Arc::clone(&vault_for_trash),
            admin_user_for_trash,
            master_key,
            move || {
                refresh_after_trash();
            },
        );
        dialog.present();
    });
}

/// Setup the sort button handlers
/// 
/// Connects click handlers to the sort buttons (recent, title, risk).
#[allow(unused_variables)]
#[allow(dead_code)]
pub fn setup_sort_button_handlers(
    _sort_recent_button: &gtk4::Button,
    _sort_title_button: &gtk4::Button,
    _sort_risk_button: &gtk4::Button,
    _secret_flow: gtk4::FlowBox,
    _filter_runtime: crate::ui::windows::main_window::types::FilterRuntime,
) {
    // TODO: Implement the actual sort button handlers
}

/// Update the visual state of sort buttons
#[allow(unused_variables)]
#[allow(dead_code)]
pub fn update_sort_button_states(
    _recent_button: &gtk4::Button,
    _title_button: &gtk4::Button,
    _risk_button: &gtk4::Button,
    _selected: SecretSortMode,
) {
    // TODO: Implement the actual button state updates
    // This would involve adding/removing CSS classes based on the selected sort mode
}

/// Apply the current filters to the secret flow
#[allow(unused_variables)]
#[allow(dead_code)]
pub fn apply_filters(
    _flow: &gtk4::FlowBox,
    _filter_runtime: &crate::ui::windows::main_window::types::FilterRuntime,
) {
    // TODO: Implement the actual filter application logic
    // This would trigger the flow box to re-filter its children
    // flow.invalidate_filter();
    // flow.invalidate_sort();
}

/// Setup the multivault toggle handler
/// 
/// Toggles between global search and vault-specific search.
#[allow(unused_variables)]
pub fn setup_multivault_toggle_handler(
    _multivault_toggle: &gtk4::ToggleButton,
    _is_global_search: Rc<Cell<bool>>,
    _filter_runtime: crate::ui::windows::main_window::types::FilterRuntime,
    _search_entry: gtk4::SearchEntry,
    _global_search_reload: Rc<dyn Fn(bool)>,
) {
    // TODO: Implement the actual toggle handler
}

/// Setup the search entry handlers
/// 
/// Handles search text changes and Enter key activation.
#[allow(unused_variables)]
#[allow(dead_code)]
pub fn setup_search_entry_handlers(
    _search_entry: &gtk4::SearchEntry,
    _secret_flow: gtk4::FlowBox,
    _filter_runtime: crate::ui::windows::main_window::types::FilterRuntime,
) {
    // TODO: Implement the actual search handlers
}

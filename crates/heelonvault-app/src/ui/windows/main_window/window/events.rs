//! Events module - Event handlers for main window
//!
//! This module contains all event handlers and callback setup functions.
//! Each handler function takes the necessary state and widgets as parameters.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use sqlx::SqlitePool;

use super::super::auto_lock;
use crate::ui::windows::main_window::types::SecretQuickActions;
#[cfg(feature = "premium")]
use heelonvault_premium::services::audit_report_service::ReportError;
#[cfg(feature = "premium")]
use heelonvault_premium::services::license_service::LicenseService;
#[cfg(feature = "premium")]
use heelonvault_premium::services::audit_report_service::AuditReportService;
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
#[allow(dead_code)]
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

/// Setup the key controller for auto-lock and keyboard shortcuts
/// 
/// Handles Ctrl+F (focus search), Ctrl+C/L/U (copy actions), and arrow key navigation.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn setup_key_controller(
    window: &adw::ApplicationWindow,
    search_entry: &gtk4::SearchEntry,
    secret_flow: &gtk4::FlowBox,
    actions_by_widget: Rc<RefCell<HashMap<String, SecretQuickActions>>>,
    auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: Rc<Cell<bool>>,
    auto_lock_timeout_secs: Rc<Cell<u64>>,
    on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    session_master_key: Rc<RefCell<Vec<u8>>>,
) {
    
    let key_controller = gtk4::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let window_for_key = window.clone();
    let search_entry_for_key = search_entry.clone();
    let flow_for_key = secret_flow.clone();
    let actions_for_key = actions_by_widget.clone();
    let source_for_key = Rc::clone(&auto_lock_source);
    let armed_for_key = Rc::clone(&auto_lock_armed);
    let timeout_for_key = Rc::clone(&auto_lock_timeout_secs);
    let callback_for_key = Rc::clone(&on_auto_lock);
    let session_for_key = Rc::clone(&session_master_key);
    
    key_controller.connect_key_pressed(move |_controller, key, _keycode, state| {
        auto_lock::reset_auto_lock_timer(
            &window_for_key,
            &source_for_key,
            &armed_for_key,
            timeout_for_key.get(),
            &callback_for_key,
            &session_for_key,
        );

        let ctrl_or_meta = state.contains(gtk4::gdk::ModifierType::CONTROL_MASK)
            || state.contains(gtk4::gdk::ModifierType::META_MASK);

        if ctrl_or_meta {
            if matches!(key, gtk4::gdk::Key::f | gtk4::gdk::Key::F) {
                search_entry_for_key.grab_focus();
                return glib::Propagation::Stop;
            }

            if matches!(
                key,
                gtk4::gdk::Key::c
                    | gtk4::gdk::Key::C
                    | gtk4::gdk::Key::l
                    | gtk4::gdk::Key::L
                    | gtk4::gdk::Key::u
                    | gtk4::gdk::Key::U
            )
                && let Some(selected_child) = flow_for_key.selected_children().first().cloned()
                && let Some(card_widget) = selected_child.child()
            {
                let widget_key = card_widget.widget_name().to_string();
                if let Some(actions) = actions_for_key.borrow().get(&widget_key) {
                    match key {
                        gtk4::gdk::Key::c | gtk4::gdk::Key::C => {
                            actions.copy_password.emit_clicked();
                            return glib::Propagation::Stop;
                        }
                        gtk4::gdk::Key::l | gtk4::gdk::Key::L => {
                            if let Some(button) = &actions.copy_login {
                                button.emit_clicked();
                                return glib::Propagation::Stop;
                            }
                        }
                        gtk4::gdk::Key::u | gtk4::gdk::Key::U => {
                            if let Some(button) = &actions.open_url {
                                button.emit_clicked();
                                return glib::Propagation::Stop;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if matches!(
            key,
            gtk4::gdk::Key::Left
                | gtk4::gdk::Key::Right
                | gtk4::gdk::Key::Up
                | gtk4::gdk::Key::Down
        ) {
            if search_entry_for_key.has_focus() {
                return glib::Propagation::Proceed;
            }

            let focus_is_in_grid = gtk4::prelude::GtkWindowExt::focus(&window_for_key)
                .is_some_and(|focus| {
                    let mut current = Some(focus);
                    while let Some(widget) = current {
                        if widget == flow_for_key.clone().upcast::<gtk4::Widget>() {
                            return true;
                        }
                        current = widget.parent();
                    }
                    false
                });

            let has_selected_child = !flow_for_key.selected_children().is_empty();
            if focus_is_in_grid || has_selected_child {
                let mut visible_children: Vec<gtk4::FlowBoxChild> = Vec::new();
                let mut cursor = flow_for_key.first_child();
                while let Some(widget) = cursor {
                    let next = widget.next_sibling();
                    if let Ok(flow_child) = widget.downcast::<gtk4::FlowBoxChild>()
                        && flow_child.is_visible()
                    {
                        visible_children.push(flow_child);
                    }
                    cursor = next;
                }

                if visible_children.is_empty() {
                    return glib::Propagation::Proceed;
                }

                let selected_idx = flow_for_key
                    .selected_children()
                    .first()
                    .and_then(|selected| {
                        visible_children
                            .iter()
                            .position(|child| child == selected)
                    })
                    .unwrap_or(0);

                let per_line = usize::try_from(flow_for_key.max_children_per_line())
                    .unwrap_or(1)
                    .max(1);
                let target_idx = match key {
                    gtk4::gdk::Key::Left => selected_idx.saturating_sub(1),
                    gtk4::gdk::Key::Right => {
                        (selected_idx.saturating_add(1)).min(visible_children.len() - 1)
                    }
                    gtk4::gdk::Key::Up => selected_idx.saturating_sub(per_line),
                    gtk4::gdk::Key::Down => {
                        (selected_idx.saturating_add(per_line)).min(visible_children.len() - 1)
                    }
                    _ => selected_idx,
                };

                flow_for_key.select_child(&visible_children[target_idx]);
                visible_children[target_idx].grab_focus();
                return glib::Propagation::Stop;
            }
        }

        glib::Propagation::Proceed
    });
    window.add_controller(key_controller);
}

/// Setup the motion controller for auto-lock reset
/// 
/// Resets the auto-lock timer when the mouse moves.
#[allow(dead_code)]
pub fn setup_motion_controller(
    window: &adw::ApplicationWindow,
    auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: Rc<Cell<bool>>,
    auto_lock_timeout_secs: Rc<Cell<u64>>,
    on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    session_master_key: Rc<RefCell<Vec<u8>>>,
) {
    let motion_controller = gtk4::EventControllerMotion::new();
    let window_for_motion = window.clone();
    let source_for_motion = Rc::clone(&auto_lock_source);
    let armed_for_motion = Rc::clone(&auto_lock_armed);
    let timeout_for_motion = Rc::clone(&auto_lock_timeout_secs);
    let callback_for_motion = Rc::clone(&on_auto_lock);
    let session_for_motion = Rc::clone(&session_master_key);
    motion_controller.connect_motion(move |_controller, _x, _y| {
        auto_lock::reset_auto_lock_timer(
            &window_for_motion,
            &source_for_motion,
            &armed_for_motion,
            timeout_for_motion.get(),
            &callback_for_motion,
            &session_for_motion,
        );
    });
    window.add_controller(motion_controller);
}

/// Setup the certification popover and handlers (premium feature)
/// 
/// Creates the certification menu with report buttons (24h, 7d, 30d) and diagnostics.
#[cfg(feature = "premium")]
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn setup_certification_handlers(
    certification_menu_button: &gtk4::Button,
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    license_service: Arc<LicenseService>,
    audit_report_service: Arc<AuditReportService>,
    report_customer_name: String,
) {
    use heelonvault_core::services::audit_report_provider::ReportError;
    
    let certification_popover = gtk4::Popover::new();
    certification_popover.set_has_arrow(true);
    certification_popover.set_autohide(true);
    
    let certification_menu_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(4)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    certification_menu_box.add_css_class("profile-login-history-popover");
    
    let report_24h_menu_button = crate::ui::windows::main_window::certification::build_certification_menu_item(
        "document-save-symbolic",
        "Rapport 24h",
    );
    let report_7d_menu_button = crate::ui::windows::main_window::certification::build_certification_menu_item(
        "document-save-symbolic",
        "Rapport 7 jours",
    );
    let report_30d_menu_button = crate::ui::windows::main_window::certification::build_certification_menu_item(
        "document-save-symbolic",
        "Rapport 30 jours",
    );
    let diagnostics_menu_button = crate::ui::windows::main_window::certification::build_certification_menu_item(
        "emblem-system-symbolic",
        "Vérifier l'état de signature",
    );
    
    certification_menu_box.append(&report_24h_menu_button);
    certification_menu_box.append(&report_7d_menu_button);
    certification_menu_box.append(&report_30d_menu_button);
    certification_menu_box.append(&diagnostics_menu_button);
    certification_popover.set_child(Some(&certification_menu_box));
    
    // Check if certification is enabled
    let certification_enabled = license_service
        .get_cached()
        .map(|license| matches!(license.tier, heelonvault_core::models::LicenseTier::Professional))
        .unwrap_or(false);
    
    certification_menu_button.set_sensitive(certification_enabled);
    if !certification_enabled {
        certification_menu_button
            .set_tooltip_text(Some("Certifier & Exporter (licence Pro requise)"));
    }
    certification_menu_button.set_popover(Some(&certification_popover));
    
    // Setup launch_signed_report closure
    let window_for_report = window.clone();
    let toast_overlay_for_report = toast_overlay.clone();
    let report_service_for_click = Arc::clone(&audit_report_service);
    let customer_name_for_click = report_customer_name.clone();
    let license_service_for_diag = Arc::clone(&license_service);
    let window_for_diag = window.clone();
    
    let launch_signed_report: Rc<dyn Fn(i64)> = Rc::new({
        let report_service = Arc::clone(&report_service_for_click);
        let customer_name = customer_name_for_click.clone();
        let report_window = window_for_report.clone();
        let report_toast_overlay = toast_overlay_for_report.clone();
        move |days| {
            report_toast_overlay
                .add_toast(adw::Toast::new("Génération du rapport signé en cours..."));

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let report_service_for_task = Arc::clone(&report_service);
            let customer_for_task = customer_name.clone();
            std::thread::spawn(move || {
                let result = report_service_for_task
                    .generate_audit_report(customer_for_task.as_str(), days);
                let _ = sender.send(result);
            });

            let window_for_result = report_window.clone();
            let toast_overlay_for_result = report_toast_overlay.clone();
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(report)) => {
                        toast_overlay_for_result.add_toast(adw::Toast::new(
                            format!(
                                "Rapport certifié généré (SHA-256: {})",
                                report.hash_prefix()
                            )
                            .as_str(),
                        ));
                        super::super::MainWindow::show_feedback_dialog(
                            &window_for_result,
                            "Rapport signé généré",
                            format!("Rapport PDF signé généré avec succès:\n{}", report.path)
                                .as_str(),
                        );
                    }
                    Ok(Err(ReportError::LicenseRequired)) => {
                        toast_overlay_for_result.add_toast(adw::Toast::new(
                            "Le rapport signé nécessite une licence Pro.",
                        ));
                    }
                    Ok(Err(ReportError::SigningKeyMissing)) => {
                        toast_overlay_for_result.add_toast(adw::Toast::new(
                            "Clé de certification indisponible. Ouvrez la Console de Confiance.",
                        ));
                    }
                    Ok(Err(error)) => super::super::MainWindow::show_feedback_dialog(
                        &window_for_result,
                        "Erreur de génération",
                        format!("Impossible de générer le rapport: {}", error).as_str(),
                    ),
                    Err(_) => super::super::MainWindow::show_feedback_dialog(
                        &window_for_result,
                        "Erreur de génération",
                        "La génération du rapport a été interrompue.",
                    ),
                }
            });
        }
    });
    
    // Setup button handlers
    let certification_popover_for_24h = certification_popover.clone();
    let launch_signed_report_for_24h = Rc::clone(&launch_signed_report);
    report_24h_menu_button.connect_clicked({
        let launch_signed_report = Rc::clone(&launch_signed_report_for_24h);
        let certification_popover = certification_popover_for_24h.clone();
        move |_| {
            certification_popover.popdown();
            launch_signed_report(1)
        }
    });
    
    let certification_popover_for_7d = certification_popover.clone();
    let launch_signed_report_for_7d = Rc::clone(&launch_signed_report);
    report_7d_menu_button.connect_clicked({
        let launch_signed_report = Rc::clone(&launch_signed_report_for_7d);
        let certification_popover = certification_popover_for_7d.clone();
        move |_| {
            certification_popover.popdown();
            launch_signed_report(7)
        }
    });
    
    let certification_popover_for_30d = certification_popover.clone();
    let launch_signed_report_for_30d = Rc::clone(&launch_signed_report);
    report_30d_menu_button.connect_clicked({
        let launch_signed_report = Rc::clone(&launch_signed_report_for_30d);
        let certification_popover = certification_popover_for_30d.clone();
        move |_| {
            certification_popover.popdown();
            launch_signed_report(30)
        }
    });
    
    let certification_popover_for_diag = certification_popover.clone();
    let license_service_for_handler = Arc::clone(&license_service_for_diag);
    let window_for_handler = window_for_diag.clone();
    diagnostics_menu_button.connect_clicked({
        let certification_popover = certification_popover_for_diag.clone();
        let report_window = window_for_handler.clone();
        let license_service = Arc::clone(&license_service_for_handler);
        move |_| {
            certification_popover.popdown();
            crate::ui::windows::main_window::certification::show_certification_diagnostics_dialog(
                &report_window,
                Arc::clone(&license_service),
                Rc::new(move |window: &adw::ApplicationWindow, title: &str, message: &str| {
                    super::super::MainWindow::show_feedback_dialog(window, title, message);
                }),
            );
        }
    });
}

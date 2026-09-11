//! Core module - Main window orchestration
//!
//! This module orchestrates the construction and setup of the main window.
//! It combines:
//! - UI construction from views.rs
//! - Event handlers from events.rs
//! - State management
//!
//! This is the main entry point for building the main window.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::AdwApplicationWindowExt;
use sqlx::SqlitePool;
use tokio::runtime::Handle;
use uuid::Uuid;

use heelonvault_core::repositories::user_repository::SqlxUserRepository;
use heelonvault_core::services::audit_service::AuditService;
use heelonvault_core::services::crypto_service::CryptoServiceImpl;
use heelonvault_core::services::pin_cache_service::PinCache;
use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::user_service::UserService;
use heelonvault_core::services::vault_service::VaultService;
#[cfg(feature = "premium")]
use heelonvault_premium::services::audit_report_service::AuditReportService;
#[cfg(feature = "premium")]
use heelonvault_premium::services::license_service::LicenseService;

use super::events;
use super::views;
#[allow(unused_imports)]
use crate::ui::window_sizing;

/// Default auto-lock timeout in seconds (12 hours)
const DEFAULT_AUTO_LOCK_TIMEOUT_SECS: u64 = 12 * 3600;

/// Build the main window with all its components and event handlers.
///
/// This function orchestrates:
/// 1. State initialization
/// 2. UI construction via views.rs
/// 3. Event handler setup via events.rs
/// 4. Final assembly and return of MainWindow
#[allow(clippy::too_many_arguments)]
pub fn build_main_window<
    TSecret,
    TVault,
    TUser,
    TAdmin,
    TTeam,
    TTotp,
    TPolicy,
    TBackup,
    TBackupApp,
    TImport,
>(
    application: &adw::Application,
    runtime_handle: Handle,
    _secret_service: Arc<TSecret>,
    _vault_service: Arc<TVault>,
    _user_service: Arc<TUser>,
    _admin_service: Arc<TAdmin>,
    _team_service: Arc<TTeam>,
    _totp_service: Arc<TTotp>,
    _auth_policy_service: Arc<TPolicy>,
    _backup_service: Arc<TBackup>,
    _backup_app_service: Arc<TBackupApp>,
    _import_service: Arc<TImport>,
    audit_service: Arc<AuditService>,
    #[cfg(feature = "premium")] _license_service: Arc<LicenseService>,
    database_pool: SqlitePool,
    _database_path: PathBuf,
    admin_user_id: Uuid,
    admin_master_key: Vec<u8>,
    connected_identity_label: String,
    license_badge_text: String,
    is_admin: bool,
) -> super::super::MainWindow
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
    TUser: UserService + Send + Sync + 'static,
    TAdmin: heelonvault_core::services::admin_service::AdminService + Send + Sync + 'static,
    TTeam: heelonvault_core::services::team_service::TeamService + Send + Sync + 'static,
    TTotp: heelonvault_core::services::totp_service::TotpService + Send + Sync + 'static,
    TPolicy: heelonvault_core::services::auth_policy_service::AuthPolicyService + Send + Sync + 'static,
    TBackup: heelonvault_core::services::backup_service::BackupService + Send + Sync + 'static,
    TBackupApp: heelonvault_core::services::backup_application_service::BackupApplicationService + Send + Sync + 'static,
    TImport: heelonvault_core::services::import_service::ImportService + Send + Sync + 'static,
{
    // ── 1. Initialize state ───────────────────────────────────────────────────
    let auto_lock_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let auto_lock_armed = Rc::new(Cell::new(false));
    let auto_lock_timeout_secs = Rc::new(Cell::new(DEFAULT_AUTO_LOCK_TIMEOUT_SECS));
    let session_master_key = Rc::new(RefCell::new(admin_master_key));
    let _active_vault_id: Rc<RefCell<Option<Uuid>>> = Rc::new(RefCell::new(None));
    let _is_global_search = Rc::new(Cell::new(false));
    let on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let on_pin_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let on_logout: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let on_pin_state_cb: Rc<RefCell<Option<Rc<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));
    let pin_cache: Rc<RefCell<Option<PinCache>>> = Rc::new(RefCell::new(None));
    let critical_ops_in_flight = Rc::new(Cell::new(0_u32));
    let refresh_entries: Rc<dyn Fn()> = Rc::new(|| {});

    #[cfg(feature = "premium")]
    let audit_report_service = Arc::new(AuditReportService::new(
        Arc::clone(&license_service),
        runtime_handle.clone(),
        database_pool.clone(),
    ));
    #[cfg(feature = "premium")]
    let report_customer_name = super::MainWindow::professional_customer_name(license_badge_text.as_str())
        .unwrap_or_else(|| "CLIENT".to_string());
    #[cfg(feature = "premium")]
    {
        events::setup_certification_handlers(
            &sidebar_panel.certification_menu_button,
            &window,
            &toast_overlay,
            Arc::clone(&license_service),
            Arc::clone(&audit_report_service),
            report_customer_name.clone(),
        );
    }

    // Create user_repo and crypto_service for profile view
    let _user_repo = Arc::new(SqlxUserRepository::new(database_pool.clone()));
    let _crypto_service = Arc::new(CryptoServiceImpl::default());

    // ── 2. Build UI Components ─────────────────────────────────────────────────
    
    // Build main window
    let window = views::build_main_window(application);
    
    // Build header bar
    let (header_bar, title_box, _logo, _title_label, _header_plan_badge, _header_license_badge) = 
        views::build_header_bar(&license_badge_text);
    
    // Build root container
    let (root, toast_overlay) = views::build_root_container();
    
    // Build profile button and popover
    let (profile_button, profile_popover, _profile_title, login_history_list) = 
        views::build_profile_button(&connected_identity_label);
    
    // Build PIN status badge
    let (header_pin_btn, _header_pin_label) = views::build_pin_status_badge();
    
    // Build panic button
    let (panic_button, _panic_lbl) = views::build_panic_button();
    
    // Build user identity box
    let (user_identity_box, admin_badge, add_button, trash_button) = 
        views::build_user_identity_box(&profile_button, is_admin);
    
    // Build center and sidebar panels from existing modules
    let center_panel = crate::ui::windows::main_window::center::build_center_panel();
    let sidebar_panel = crate::ui::windows::main_window::sidebar::build_sidebar_panel();
    
    // Build filter runtime for search and filtering
    use crate::ui::windows::main_window::types::FilterRuntime;
    use crate::ui::windows::main_window::{AuditFilter, SecretCategoryFilter, SecretKind, SecretSortMode};
    use std::collections::HashMap;
    let filter_runtime = FilterRuntime {
        meta_by_widget: Rc::new(RefCell::new(HashMap::new())),
        actions_by_widget: Rc::new(RefCell::new(HashMap::new())),
        search_text: Rc::new(RefCell::new(String::new())),
        selected_category: Rc::new(Cell::new(SecretCategoryFilter::All)),
        selected_audit: Rc::new(Cell::new(AuditFilter::All)),
        selected_sort: Rc::new(Cell::new(SecretSortMode::Recent)),
        audit_all_count_label: sidebar_panel.audit_all_badge.clone(),
        audit_weak_count_label: sidebar_panel.audit_weak_badge.clone(),
        audit_duplicate_count_label: sidebar_panel.audit_duplicate_badge.clone(),
        total_count_label: center_panel.status_total_badge.clone(),
        non_compliant_count_label: center_panel.status_non_compliant_badge.clone(),
        filtered_status_page: center_panel.filtered_status_page.clone(),
    };
    
    // Set filter and sort functions for secret flow
    let runtime_for_flow_filter = filter_runtime.clone();
    center_panel.secret_flow.set_filter_func(move |child| {
        let Some(content) = child.child() else {
            return false;
        };
        let key = content.widget_name().to_string();
        let store = runtime_for_flow_filter.meta_by_widget.borrow();
        let Some(meta) = store.get(&key) else {
            return true;
        };

        let query = runtime_for_flow_filter.search_text.borrow().to_string();
        let terms = super::super::MainWindow::parse_search_terms(query.as_str());
        let matches_query = terms.is_empty()
            || terms
                .iter()
                .all(|term| super::super::MainWindow::matches_search_term(meta, term));

        let matches_category = match runtime_for_flow_filter.selected_category.get() {
            SecretCategoryFilter::All => true,
            SecretCategoryFilter::Password => meta.kind == SecretKind::Password,
            SecretCategoryFilter::ApiToken => meta.kind == SecretKind::ApiToken,
            SecretCategoryFilter::SshKey => meta.kind == SecretKind::SshKey,
            SecretCategoryFilter::SecureDocument => meta.kind == SecretKind::SecureDocument,
        };

        let matches_audit = match runtime_for_flow_filter.selected_audit.get() {
            AuditFilter::All => true,
            AuditFilter::Weak => meta.is_weak,
            AuditFilter::Duplicate => meta.is_duplicate,
        };

        matches_query && matches_category && matches_audit
    });
    
    let runtime_for_flow_sort = filter_runtime.clone();
    center_panel.secret_flow.set_sort_func(move |left, right| {
        let left_key = left
            .child()
            .map(|child| child.widget_name().to_string())
            .unwrap_or_default();
        let right_key = right
            .child()
            .map(|child| child.widget_name().to_string())
            .unwrap_or_default();

        let store = runtime_for_flow_sort.meta_by_widget.borrow();
        let Some(left_meta) = store.get(&left_key) else {
            return left_key.cmp(&right_key).into();
        };
        let Some(right_meta) = store.get(&right_key) else {
            return left_key.cmp(&right_key).into();
        };

        match runtime_for_flow_sort.selected_sort.get() {
            SecretSortMode::Recent => left_meta
                .original_rank
                .cmp(&right_meta.original_rank)
                .into(),
            SecretSortMode::Title => left_meta
                .title_text
                .cmp(&right_meta.title_text)
                .then(left_meta.original_rank.cmp(&right_meta.original_rank))
                .into(),
            SecretSortMode::Risk => {
                let left_score =
                    usize::from(left_meta.is_weak) + usize::from(left_meta.is_duplicate);
                let right_score =
                    usize::from(right_meta.is_weak) + usize::from(right_meta.is_duplicate);
                right_score
                    .cmp(&left_score)
                    .then(left_meta.title_text.cmp(&right_meta.title_text))
                    .then(left_meta.original_rank.cmp(&right_meta.original_rank))
                    .into()
            }
        }
    });
    
    // Build shell content (search entry, multivault toggle, etc.)
    let content_shell = crate::ui::windows::main_window::shell::build_content_shell(
        &sidebar_panel.frame,
        &center_panel.frame,
    );
    let search_entry = content_shell.search_entry;
    let _multivault_toggle = content_shell.multivault_toggle;
    
    // ── 3. Setup Event Handlers ───────────────────────────────────────────────
    
    // Window close handler
    events::setup_window_close_handler(
        &window,
        Rc::clone(&critical_ops_in_flight),
        Rc::clone(&on_logout),
    );
    
    // Profile popover handlers
    events::setup_profile_popover_handlers(
        &profile_popover,
        &profile_button,
        runtime_handle.clone(),
        database_pool.clone(),
        admin_user_id,
        login_history_list,
    );
    
    // PIN badge handler
    events::setup_pin_badge_handler(
        &header_pin_btn,
        window.clone(),
        Rc::clone(&session_master_key),
        Rc::clone(&pin_cache),
        Rc::clone(&on_pin_state_cb),
        admin_user_id,
    );
    
    // Panic button handler
    events::setup_panic_button_handler(&panic_button, window.clone());
    
    // Add button handler - TODO: extract from new_body.inc
    // Needs open_editor callback which depends on more extraction
    
    // Trash button handler
    let refresh_entries_clone = Rc::clone(&refresh_entries);
    events::setup_trash_button_handler(
        &trash_button,
        application,
        window.clone(),
        runtime_handle.clone(),
        Arc::clone(&_secret_service),
        Arc::clone(&_vault_service),
        admin_user_id,
        Rc::clone(&session_master_key),
        refresh_entries_clone,
    );
    
    // Key controller for auto-lock and keyboard shortcuts
    events::setup_key_controller(
        &window,
        &search_entry,
        &center_panel.secret_flow,
        Rc::clone(&filter_runtime.actions_by_widget),
        Rc::clone(&auto_lock_source),
        Rc::clone(&auto_lock_armed),
        Rc::clone(&auto_lock_timeout_secs),
        Rc::clone(&on_auto_lock),
        Rc::clone(&session_master_key),
    );
    
    // Motion controller for auto-lock reset
    events::setup_motion_controller(
        &window,
        Rc::clone(&auto_lock_source),
        Rc::clone(&auto_lock_armed),
        Rc::clone(&auto_lock_timeout_secs),
        Rc::clone(&on_auto_lock),
        Rc::clone(&session_master_key),
    );
    
    // Sort button handlers - TODO: extract from new_body.inc
    // The filter and sort functions are now set directly on secret_flow above
    // events::setup_sort_button_handlers(
    //     &center_panel.sort_recent_button,
    //     &center_panel.sort_title_button,
    //     &center_panel.sort_risk_button,
    //     center_panel.secret_flow.clone(),
    //     filter_runtime.clone(),
    // );
    
    // Search entry handlers - TODO: extract from new_body.inc
    // events::setup_search_entry_handlers(
    //     &search_entry,
    //     center_panel.secret_flow.clone(),
    //     filter_runtime.clone(),
    // );
    
    // Multivault toggle handler - TODO: extract from new_body.inc
    // events::setup_multivault_toggle_handler(
    //     &multivault_toggle,
    //     Rc::clone(&is_global_search),
    //     filter_runtime.clone(),
    //     search_entry.clone(),
    //     Rc::new(|_is_global| {}),
    // );

    // ── 4. Assemble the UI ────────────────────────────────────────────────────
    
    // Add header pin button to user identity box
    user_identity_box.append(&header_pin_btn);
    
    // Add admin badge if applicable
    if is_admin {
        user_identity_box.append(&admin_badge);
    }
    
    // Add buttons to header bar
    header_bar.pack_start(&add_button);
    header_bar.pack_start(&trash_button);
    header_bar.pack_end(&user_identity_box);
    header_bar.pack_end(&panic_button);
    
    // Add title box to header bar
    header_bar.set_title_widget(Some(&title_box));
    
    // Build final UI hierarchy
    let content = content_shell.container;
    root.append(&header_bar);
    root.append(&content);
    toast_overlay.set_child(Some(&root));
    window.set_content(Some(&toast_overlay));

    // ── 5. Return MainWindow ─────────────────────────────────────────────────
    super::super::MainWindow {
        window,
        secret_flow: center_panel.secret_flow,
        refresh_entries,
        auto_lock_timeout_secs,
        auto_lock_source,
        auto_lock_armed,
        session_master_key,
        pin_cache,
        on_auto_lock,
        on_pin_lock,
        on_logout,
        on_pin_state_cb,
        session_user_id: admin_user_id,
        audit_service: Rc::new(audit_service),
    }
}

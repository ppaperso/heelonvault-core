//! Main window orchestration.
//!
//! This module owns the construction order of the window and nothing else: UI comes from
//! `views.rs` and the panel builders, behaviour comes from `events.rs` and the feature
//! modules (`editor`, `refresh`, `vault_list`, `navigation`, `pin_badge`, `i18n_refresh`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
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

use super::{editor, events, i18n_refresh, navigation, pin_badge, refresh, vault_list, views};
use crate::ui::dialogs::add_edit_dialog::DialogMode;
use crate::ui::windows::main_window::types::FilterRuntime;
use crate::ui::windows::main_window::{
    AuditFilter, SecretCategoryFilter, SecretKind, SecretSortMode, center, search_filter, shell,
    sidebar,
};

/// Default auto-lock timeout in seconds (12 hours)
const DEFAULT_AUTO_LOCK_TIMEOUT_SECS: u64 = 12 * 3600;

/// Build the main window with all its components and event handlers.
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
    secret_service: Arc<TSecret>,
    vault_service: Arc<TVault>,
    user_service: Arc<TUser>,
    admin_service: Arc<TAdmin>,
    team_service: Arc<TTeam>,
    totp_service: Arc<TTotp>,
    auth_policy_service: Arc<TPolicy>,
    backup_service: Arc<TBackup>,
    backup_app_service: Arc<TBackupApp>,
    import_service: Arc<TImport>,
    audit_service: Arc<AuditService>,
    #[cfg(feature = "premium")] license_service: Arc<LicenseService>,
    database_pool: SqlitePool,
    database_path: PathBuf,
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
    TPolicy:
        heelonvault_core::services::auth_policy_service::AuthPolicyService + Send + Sync + 'static,
    TBackup: heelonvault_core::services::backup_service::BackupService + Send + Sync + 'static,
    TBackupApp: heelonvault_core::services::backup_application_service::BackupApplicationService
        + Send
        + Sync
        + 'static,
    TImport: heelonvault_core::services::import_service::ImportService + Send + Sync + 'static,
{
    // ── 1. Session and UI state ───────────────────────────────────────────────
    let auto_lock_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let auto_lock_armed = Rc::new(Cell::new(false));
    let auto_lock_timeout_secs = Rc::new(Cell::new(DEFAULT_AUTO_LOCK_TIMEOUT_SECS));
    let session_master_key = Rc::new(RefCell::new(admin_master_key));
    let active_vault_id: Rc<RefCell<Option<Uuid>>> = Rc::new(RefCell::new(None));
    let is_global_search = Rc::new(Cell::new(false));
    let on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let on_pin_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let on_logout: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let on_pin_state_cb: Rc<RefCell<Option<Rc<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));
    let pin_cache: Rc<RefCell<Option<PinCache>>> = Rc::new(RefCell::new(None));
    let critical_ops_in_flight = Rc::new(Cell::new(0_u32));
    let show_passwords_in_edit = Rc::new(Cell::new(false));
    let vault_selection_sync = Rc::new(Cell::new(false));
    let default_vault_creation = Rc::new(Cell::new(false));

    // Deferred callbacks: these break construction cycles where a widget needs a callback
    // that can only be built once that same widget exists.
    let editor_launcher: Rc<RefCell<Option<Rc<dyn Fn(DialogMode)>>>> = Rc::new(RefCell::new(None));
    let refresh_after_mutation: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let i18n_refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let user_repo = Arc::new(SqlxUserRepository::new(database_pool.clone()));
    let crypto_service = Arc::new(CryptoServiceImpl::default());

    // ── 2. UI construction ────────────────────────────────────────────────────
    let window = views::build_main_window(application);
    let (header_bar, title_box, _logo, _title_label, _header_plan_badge, _header_license_badge) =
        views::build_header_bar(&license_badge_text);
    let (root, toast_overlay) = views::build_root_container();
    let (profile_button, profile_popover, profile_title, login_history_list) =
        views::build_profile_button(&connected_identity_label);
    let (header_pin_btn, header_pin_label) = views::build_pin_status_badge();
    let (panic_button, panic_label) = views::build_panic_button();
    let (user_identity_box, admin_badge, add_button, trash_button) =
        views::build_user_identity_box(&profile_button, is_admin);

    let center_panel = center::build_center_panel();
    let sidebar_panel = sidebar::build_sidebar_panel();

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

    install_flow_filter_and_sort(&center_panel.secret_flow, &filter_runtime);

    let content_shell = shell::build_content_shell(&sidebar_panel.frame, &center_panel.frame);
    let search_entry = content_shell.search_entry;
    let multivault_toggle = content_shell.multivault_toggle;
    let shell_refresh_i18n = content_shell.refresh_i18n;

    let editor_host = editor::build_editor_page(&center_panel.main_stack);

    // ── 3. Refresh chain ──────────────────────────────────────────────────────
    // A single reload backs both the active-vault and the cross-vault refresh.
    let secret_reload = refresh::build_secret_reload(refresh::SecretRefreshDeps {
        application: application.clone(),
        parent_window: window.clone(),
        runtime_handle: runtime_handle.clone(),
        secret_service: Arc::clone(&secret_service),
        vault_service: Arc::clone(&vault_service),
        user_id: admin_user_id,
        session_master_key: Rc::clone(&session_master_key),
        active_vault_id: Rc::clone(&active_vault_id),
        secret_flow: center_panel.secret_flow.clone(),
        stack: center_panel.stack.clone(),
        empty_title: center_panel.empty_title.clone(),
        empty_copy: center_panel.empty_copy.clone(),
        toast_overlay: toast_overlay.clone(),
        filter_runtime: filter_runtime.clone(),
        editor_launcher: Rc::clone(&editor_launcher),
    });

    let refresh_secrets: Rc<dyn Fn()> = {
        let reload = Rc::clone(&secret_reload);
        let is_global_search = Rc::clone(&is_global_search);
        Rc::new(move || reload(is_global_search.get()))
    };

    // The full refresh reloads the vault sections, which chain into the secret list.
    let refresh_entries = vault_list::build_refresh_vault_sections(vault_list::VaultListDeps {
        window: window.clone(),
        runtime_handle: runtime_handle.clone(),
        secret_service: Arc::clone(&secret_service),
        vault_service: Arc::clone(&vault_service),
        user_id: admin_user_id,
        session_master_key: Rc::clone(&session_master_key),
        active_vault_id: Rc::clone(&active_vault_id),
        my_vaults_list: sidebar_panel.my_vaults_list.clone(),
        shared_vaults_title: sidebar_panel.shared_vaults_title.clone(),
        shared_vaults_list: sidebar_panel.shared_vaults_list.clone(),
        selection_sync: Rc::clone(&vault_selection_sync),
        default_vault_creation: Rc::clone(&default_vault_creation),
        refresh_secrets: Rc::clone(&refresh_secrets),
        refresh_after_mutation: Rc::clone(&refresh_after_mutation),
    });
    *refresh_after_mutation.borrow_mut() = Some(Rc::clone(&refresh_entries));

    *editor_launcher.borrow_mut() = Some(editor::build_editor_launcher(editor::EditorDeps {
        runtime_handle: runtime_handle.clone(),
        secret_service: Arc::clone(&secret_service),
        vault_service: Arc::clone(&vault_service),
        user_id: admin_user_id,
        session_master_key: Rc::clone(&session_master_key),
        show_passwords_in_edit: Rc::clone(&show_passwords_in_edit),
        main_stack: center_panel.main_stack.clone(),
        editor_host: editor_host.clone(),
        toast_overlay: toast_overlay.clone(),
        refresh_entries: Rc::clone(&refresh_entries),
    }));
    let open_editor = editor_launcher
        .borrow()
        .as_ref()
        .map(Rc::clone)
        .unwrap_or_else(|| Rc::new(|_mode| {}));

    // ── 4. Event handlers ─────────────────────────────────────────────────────
    events::setup_window_close_handler(
        &window,
        Rc::clone(&critical_ops_in_flight),
        Rc::clone(&on_logout),
    );

    events::setup_profile_popover_handlers(
        &profile_popover,
        &profile_button,
        runtime_handle.clone(),
        database_pool.clone(),
        admin_user_id,
        login_history_list,
    );

    let on_pin_state_changed = pin_badge::build_pin_state_callback(
        header_pin_label.clone(),
        header_pin_btn.clone(),
        Rc::clone(&pin_cache),
    );
    *on_pin_state_cb.borrow_mut() = Some(Rc::clone(&on_pin_state_changed));

    events::setup_pin_badge_handler(
        &header_pin_btn,
        window.clone(),
        Rc::clone(&session_master_key),
        Rc::clone(&pin_cache),
        Rc::clone(&on_pin_state_cb),
        admin_user_id,
    );

    events::setup_panic_button_handler(&panic_button, window.clone());

    events::setup_add_button_handler(
        &add_button,
        window.clone(),
        Rc::clone(&active_vault_id),
        runtime_handle.clone(),
        Arc::clone(&vault_service),
        Rc::clone(&open_editor),
        admin_user_id,
    );

    events::setup_trash_button_handler(
        &trash_button,
        application,
        window.clone(),
        runtime_handle.clone(),
        Arc::clone(&secret_service),
        Arc::clone(&vault_service),
        admin_user_id,
        Rc::clone(&session_master_key),
        Rc::clone(&refresh_entries),
    );

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

    events::setup_motion_controller(
        &window,
        Rc::clone(&auto_lock_source),
        Rc::clone(&auto_lock_armed),
        Rc::clone(&auto_lock_timeout_secs),
        Rc::clone(&on_auto_lock),
        Rc::clone(&session_master_key),
    );

    events::setup_sort_button_handlers(
        &center_panel.sort_recent_button,
        &center_panel.sort_title_button,
        &center_panel.sort_risk_button,
        center_panel.secret_flow.clone(),
        filter_runtime.clone(),
    );

    events::setup_search_entry_handlers(
        &search_entry,
        center_panel.secret_flow.clone(),
        filter_runtime.clone(),
    );

    events::setup_sidebar_filter_handlers(
        &sidebar_panel,
        &center_panel.main_stack,
        center_panel.secret_flow.clone(),
        filter_runtime.clone(),
    );

    events::setup_multivault_toggle_handler(
        &multivault_toggle,
        Rc::clone(&is_global_search),
        filter_runtime.clone(),
        search_entry.clone(),
        Rc::clone(&secret_reload),
    );

    vault_list::setup_vault_selection_handlers(
        &sidebar_panel,
        &center_panel.main_stack,
        Rc::clone(&active_vault_id),
        Rc::clone(&vault_selection_sync),
        Rc::clone(&refresh_secrets),
    );

    vault_list::setup_create_vault_button(
        &sidebar_panel.create_vault_button,
        window.clone(),
        runtime_handle.clone(),
        Arc::clone(&vault_service),
        admin_user_id,
        Rc::clone(&session_master_key),
        Rc::clone(&refresh_entries),
    );

    #[cfg(feature = "premium")]
    {
        let audit_report_service = Arc::new(AuditReportService::new(
            Arc::clone(&license_service),
            runtime_handle.clone(),
            database_pool.clone(),
        ));
        let report_customer_name =
            super::super::MainWindow::professional_customer_name(license_badge_text.as_str())
                .unwrap_or_else(|| "CLIENT".to_string());
        events::setup_certification_handlers(
            &sidebar_panel.certification_menu_button,
            &window,
            &toast_overlay,
            Arc::clone(&license_service),
            audit_report_service,
            report_customer_name,
        );
    }

    // ── 5. Secondary navigation pages ─────────────────────────────────────────
    #[cfg(feature = "premium")]
    let admin_pages = if is_admin {
        Some(navigation::setup_admin_pages(
            application,
            &window,
            runtime_handle.clone(),
            Arc::clone(&admin_service),
            Arc::clone(&team_service),
            Arc::clone(&vault_service),
            admin_user_id,
            Rc::clone(&session_master_key),
            Rc::clone(&active_vault_id),
            Rc::clone(&refresh_entries),
            &center_panel.main_stack,
            &sidebar_panel.administration_button,
            &sidebar_panel.teams_button,
        ))
    } else {
        None
    };

    // Users and teams management is a premium capability: the entries stay hidden in the
    // community build, where the services they would drive are unused.
    sidebar_panel
        .administration_button
        .set_visible(is_admin && cfg!(feature = "premium"));
    sidebar_panel
        .teams_button
        .set_visible(is_admin && cfg!(feature = "premium"));
    #[cfg(not(feature = "premium"))]
    let _ = (&admin_service, &team_service);

    let on_language_changed: Rc<dyn Fn()> = {
        let holder = Rc::clone(&i18n_refresh_holder);
        Rc::new(move || {
            let callback = holder.borrow().as_ref().map(Rc::clone);
            if let Some(callback) = callback {
                callback();
            }
        })
    };

    let profile_container = navigation::setup_profile_page(
        navigation::ProfilePageDeps {
            window: window.clone(),
            runtime_handle: runtime_handle.clone(),
            user_service: Arc::clone(&user_service),
            user_repo,
            crypto_service,
            totp_service: Arc::clone(&totp_service),
            auth_policy_service: Arc::clone(&auth_policy_service),
            backup_service: Arc::clone(&backup_service),
            backup_app_service: Arc::clone(&backup_app_service),
            import_service: Arc::clone(&import_service),
            secret_service: Arc::clone(&secret_service),
            vault_service: Arc::clone(&vault_service),
            database_path,
            user_id: admin_user_id,
            is_admin,
            profile_badge: profile_button.clone(),
            critical_ops_in_flight: Rc::clone(&critical_ops_in_flight),
            auto_lock_timeout_secs: Rc::clone(&auto_lock_timeout_secs),
            auto_lock_source: Rc::clone(&auto_lock_source),
            auto_lock_armed: Rc::clone(&auto_lock_armed),
            on_auto_lock: Rc::clone(&on_auto_lock),
            session_master_key: Rc::clone(&session_master_key),
            pin_cache: Rc::clone(&pin_cache),
            show_passwords_in_edit: Rc::clone(&show_passwords_in_edit),
            refresh_entries: Rc::clone(&refresh_entries),
            on_language_changed,
            on_pin_state_changed,
        },
        &center_panel.main_stack,
        &sidebar_panel.profile_security_button,
        build_stack_switch(&center_panel.main_stack, "users_view"),
        build_stack_switch(&center_panel.main_stack, "teams_view"),
    );

    // ── 6. Live translation ───────────────────────────────────────────────────
    let refresh_i18n = i18n_refresh::build_refresh(
        i18n_refresh::I18nTargets {
            sidebar_panel: sidebar_panel.clone(),
            center_panel: center_panel.clone(),
            shell_refresh: shell_refresh_i18n,
            profile_button: profile_button.clone(),
            profile_title,
            add_button: add_button.clone(),
            trash_button: trash_button.clone(),
            panic_button: panic_button.clone(),
            panic_label,
            profile_container,
            editor_host,
        },
        #[cfg(feature = "premium")]
        admin_pages,
    );
    *i18n_refresh_holder.borrow_mut() = Some(Rc::clone(&refresh_i18n));
    refresh_i18n();

    events::update_sort_button_states(
        &center_panel.sort_recent_button,
        &center_panel.sort_title_button,
        &center_panel.sort_risk_button,
        filter_runtime.selected_sort.get(),
    );

    // ── 7. Assemble ───────────────────────────────────────────────────────────
    user_identity_box.append(&header_pin_btn);
    if is_admin {
        user_identity_box.append(&admin_badge);
    }

    header_bar.pack_start(&add_button);
    header_bar.pack_start(&trash_button);
    header_bar.pack_end(&user_identity_box);
    header_bar.pack_end(&panic_button);
    header_bar.set_title_widget(Some(&title_box));

    root.append(&header_bar);
    root.append(&content_shell.container);
    toast_overlay.set_child(Some(&root));
    window.set_content(Some(&toast_overlay));

    // Populate vaults and secrets for the freshly opened session.
    refresh_entries();

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

/// Build a callback that brings a named stack page to the front.
fn build_stack_switch(main_stack: &gtk4::Stack, page_name: &'static str) -> Rc<dyn Fn()> {
    let stack = main_stack.clone();
    Rc::new(move || stack.set_visible_child_name(page_name))
}

/// Install the filter and sort predicates backing the secret flow box.
fn install_flow_filter_and_sort(secret_flow: &gtk4::FlowBox, filter_runtime: &FilterRuntime) {
    let runtime_for_filter = filter_runtime.clone();
    secret_flow.set_filter_func(move |child| {
        let Some(content) = child.child() else {
            return false;
        };
        let key = content.widget_name().to_string();
        let store = runtime_for_filter.meta_by_widget.borrow();
        let Some(meta) = store.get(&key) else {
            return true;
        };

        let query = runtime_for_filter.search_text.borrow().to_string();
        let terms = search_filter::parse_search_terms(query.as_str());
        let matches_query = terms.is_empty()
            || terms
                .iter()
                .all(|term| search_filter::matches_search_term(meta, term));

        let matches_category = match runtime_for_filter.selected_category.get() {
            SecretCategoryFilter::All => true,
            SecretCategoryFilter::Password => meta.kind == SecretKind::Password,
            SecretCategoryFilter::ApiToken => meta.kind == SecretKind::ApiToken,
            SecretCategoryFilter::SshKey => meta.kind == SecretKind::SshKey,
            SecretCategoryFilter::SecureDocument => meta.kind == SecretKind::SecureDocument,
        };

        let matches_audit = match runtime_for_filter.selected_audit.get() {
            AuditFilter::All => true,
            AuditFilter::Weak => meta.is_weak,
            AuditFilter::Duplicate => meta.is_duplicate,
        };

        matches_query && matches_category && matches_audit
    });

    let runtime_for_sort = filter_runtime.clone();
    secret_flow.set_sort_func(move |left, right| {
        let left_key = left
            .child()
            .map(|child| child.widget_name().to_string())
            .unwrap_or_default();
        let right_key = right
            .child()
            .map(|child| child.widget_name().to_string())
            .unwrap_or_default();

        let store = runtime_for_sort.meta_by_widget.borrow();
        let Some(left_meta) = store.get(&left_key) else {
            return left_key.cmp(&right_key).into();
        };
        let Some(right_meta) = store.get(&right_key) else {
            return left_key.cmp(&right_key).into();
        };

        match runtime_for_sort.selected_sort.get() {
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
}

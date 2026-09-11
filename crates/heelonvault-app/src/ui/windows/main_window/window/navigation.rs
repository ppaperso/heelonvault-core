//! Secondary navigation: the inline stack pages reached from the sidebar.
//!
//! Profile, users and teams are pages of the main stack rather than modal dialogs, so the
//! sidebar stays visible and the user keeps their bearings while using them.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
#[cfg(feature = "premium")]
use gtk4::{Align, Orientation};
use libadwaita as adw;
use tokio::runtime::Handle;
use uuid::Uuid;

use heelonvault_core::repositories::user_repository::SqlxUserRepository;
use heelonvault_core::services::auth_policy_service::AuthPolicyService;
use heelonvault_core::services::backup_application_service::BackupApplicationService;
use heelonvault_core::services::backup_service::BackupService;
use heelonvault_core::services::crypto_service::CryptoServiceImpl;
use heelonvault_core::services::import_service::ImportService;
use heelonvault_core::services::pin_cache_service::PinCache;
use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::totp_service::TotpService;
use heelonvault_core::services::user_service::UserService;
use heelonvault_core::services::vault_service::VaultService;

use crate::ui::windows::main_window::profile_view;

/// A sidebar-reachable stack page with a header and a back button.
#[cfg(feature = "premium")]
struct InlinePage {
    container: gtk4::ScrolledWindow,
    root: gtk4::Box,
    back_button: gtk4::Button,
    title: gtk4::Label,
    intro: gtk4::Label,
}

/// Build the shared chrome used by the users and teams pages.
#[cfg(feature = "premium")]
fn build_inline_page(title_key: &str, intro_key: &str) -> InlinePage {
    let container = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    root.add_css_class("profile-view-content");

    let header = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    header.add_css_class("profile-view-header");

    let back_button = gtk4::Button::builder()
        .label(heelonvault_core::tr!("main-view-back").as_str())
        .icon_name("go-previous-symbolic")
        .build();
    back_button.add_css_class("flat");
    back_button.add_css_class("main-inline-back-button");
    back_button.set_halign(Align::Start);

    let title = gtk4::Label::new(Some(heelonvault_core::i18n::tr(title_key).as_str()));
    title.add_css_class("title-3");
    title.add_css_class("heading");
    title.add_css_class("main-section-title");
    title.set_hexpand(true);
    title.set_halign(Align::Center);

    header.append(&back_button);
    header.append(&title);
    root.append(&header);

    let intro = gtk4::Label::new(Some(heelonvault_core::i18n::tr(intro_key).as_str()));
    intro.set_halign(Align::Start);
    intro.set_wrap(true);
    intro.add_css_class("dim-label");
    root.append(&intro);

    InlinePage {
        container,
        root,
        back_button,
        title,
        intro,
    }
}

/// Register an inline page in the stack and wire its back button to the entries view.
#[cfg(feature = "premium")]
fn install_inline_page(
    page: &InlinePage,
    content: gtk4::Widget,
    main_stack: &gtk4::Stack,
    page_name: &str,
    page_title: &str,
    open_button: &gtk4::Button,
) {
    let frame = gtk4::Frame::new(None);
    frame.add_css_class("profile-section-frame");
    frame.set_child(Some(&content));
    page.root.append(&frame);
    page.container.set_child(Some(&page.root));

    main_stack.add_titled(&page.container, Some(page_name), page_title);

    let stack_for_back = main_stack.clone();
    page.back_button.connect_clicked(move |_| {
        stack_for_back.set_visible_child_name("entries_view");
    });

    let stack_for_open = main_stack.clone();
    let page_name = page_name.to_string();
    open_button.connect_clicked(move |_| {
        stack_for_open.set_visible_child_name(page_name.as_str());
    });
}

#[allow(clippy::too_many_arguments)]
pub struct ProfilePageDeps<TUser, TTotp, TPolicy, TBackup, TBackupApp, TImport, TSecret, TVault> {
    pub window: adw::ApplicationWindow,
    pub runtime_handle: Handle,
    pub user_service: Arc<TUser>,
    pub user_repo: Arc<SqlxUserRepository>,
    pub crypto_service: Arc<CryptoServiceImpl>,
    pub totp_service: Arc<TTotp>,
    pub auth_policy_service: Arc<TPolicy>,
    pub backup_service: Arc<TBackup>,
    pub backup_app_service: Arc<TBackupApp>,
    pub import_service: Arc<TImport>,
    pub secret_service: Arc<TSecret>,
    pub vault_service: Arc<TVault>,
    pub database_path: PathBuf,
    pub user_id: Uuid,
    pub is_admin: bool,
    pub profile_badge: gtk4::MenuButton,
    pub critical_ops_in_flight: Rc<Cell<u32>>,
    pub auto_lock_timeout_secs: Rc<Cell<u64>>,
    pub auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
    pub auto_lock_armed: Rc<Cell<bool>>,
    pub on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    pub session_master_key: Rc<RefCell<Vec<u8>>>,
    pub pin_cache: Rc<RefCell<Option<PinCache>>>,
    pub show_passwords_in_edit: Rc<Cell<bool>>,
    pub refresh_entries: Rc<dyn Fn()>,
    pub on_language_changed: Rc<dyn Fn()>,
    pub on_pin_state_changed: Rc<dyn Fn(bool)>,
}

/// Build the profile page, register it in the stack and wire the sidebar entry to it.
///
/// Returns the page container so the i18n refresh can retitle it.
#[allow(clippy::too_many_arguments)]
pub fn setup_profile_page<TUser, TTotp, TPolicy, TBackup, TBackupApp, TImport, TSecret, TVault>(
    deps: ProfilePageDeps<TUser, TTotp, TPolicy, TBackup, TBackupApp, TImport, TSecret, TVault>,
    main_stack: &gtk4::Stack,
    profile_security_button: &gtk4::Button,
    on_open_users_view: Rc<dyn Fn()>,
    on_open_teams_view: Rc<dyn Fn()>,
) -> gtk4::ScrolledWindow
where
    TUser: UserService + Send + Sync + 'static,
    TTotp: TotpService + Send + Sync + 'static,
    TPolicy: AuthPolicyService + Send + Sync + 'static,
    TBackup: BackupService + Send + Sync + 'static,
    TBackupApp: BackupApplicationService + Send + Sync + 'static,
    TImport: ImportService + Send + Sync + 'static,
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let profile_view = profile_view::build_profile_view(
        deps.window,
        deps.runtime_handle,
        deps.user_service,
        deps.user_repo,
        deps.crypto_service,
        deps.totp_service,
        deps.auth_policy_service,
        deps.backup_service,
        deps.backup_app_service,
        deps.import_service,
        deps.secret_service,
        deps.vault_service,
        deps.database_path,
        deps.user_id,
        deps.is_admin,
        on_open_users_view,
        on_open_teams_view,
        deps.profile_badge,
        deps.critical_ops_in_flight,
        deps.auto_lock_timeout_secs,
        deps.auto_lock_source,
        deps.auto_lock_armed,
        deps.on_auto_lock,
        deps.session_master_key,
        deps.pin_cache,
        deps.show_passwords_in_edit,
        deps.refresh_entries,
        deps.on_language_changed,
        deps.on_pin_state_changed,
    );

    main_stack.add_titled(
        &profile_view.container,
        Some("profile_view"),
        heelonvault_core::tr!("main-profile-security").as_str(),
    );

    let stack_for_back = main_stack.clone();
    profile_view.back_button.connect_clicked(move |_| {
        stack_for_back.set_visible_child_name("entries_view");
    });

    let stack_for_open = main_stack.clone();
    profile_security_button.connect_clicked(move |_| {
        stack_for_open.set_visible_child_name("profile_view");
    });

    profile_view.container
}

/// Translatable widgets of the admin-only pages, kept so the i18n refresh can update them.
#[cfg(feature = "premium")]
pub struct AdminPages {
    users: InlinePage,
    teams: InlinePage,
}

#[cfg(feature = "premium")]
impl AdminPages {
    /// Re-apply translations to both admin pages and their stack titles.
    pub fn refresh_i18n(&self, main_stack: &gtk4::Stack) {
        self.users
            .back_button
            .set_label(heelonvault_core::tr!("main-view-back").as_str());
        self.users
            .title
            .set_text(heelonvault_core::tr!("main-users-view-title").as_str());
        self.users
            .intro
            .set_text(heelonvault_core::tr!("main-users-view-intro").as_str());
        self.teams
            .back_button
            .set_label(heelonvault_core::tr!("main-view-back").as_str());
        self.teams
            .title
            .set_text(heelonvault_core::tr!("main-teams-view-title").as_str());
        self.teams
            .intro
            .set_text(heelonvault_core::tr!("main-teams-view-intro").as_str());

        main_stack
            .page(&self.users.container)
            .set_title(heelonvault_core::tr!("main-user-nav").as_str());
        main_stack
            .page(&self.teams.container)
            .set_title(heelonvault_core::tr!("main-teams-nav").as_str());
    }
}

/// Build the admin-only users and teams pages.
///
/// Both embed the content of their management dialog inline rather than presenting it as a
/// modal window.
#[cfg(feature = "premium")]
#[allow(clippy::too_many_arguments)]
pub fn setup_admin_pages<TAdmin, TTeam, TVault>(
    application: &adw::Application,
    window: &adw::ApplicationWindow,
    runtime_handle: Handle,
    admin_service: Arc<TAdmin>,
    team_service: Arc<TTeam>,
    vault_service: Arc<TVault>,
    user_id: Uuid,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    active_vault_id: Rc<RefCell<Option<Uuid>>>,
    refresh_entries: Rc<dyn Fn()>,
    main_stack: &gtk4::Stack,
    administration_button: &gtk4::Button,
    teams_button: &gtk4::Button,
) -> AdminPages
where
    TAdmin: heelonvault_core::services::admin_service::AdminService + Send + Sync + 'static,
    TTeam: heelonvault_core::services::team_service::TeamService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    use crate::ui::dialogs::manage_teams_dialog::ManageTeamsDialog;
    use crate::ui::dialogs::manage_users_dialog::ManageUsersDialog;

    let users = build_inline_page("main-users-view-title", "main-users-view-intro");
    let users_dialog = ManageUsersDialog::new(
        application,
        window,
        runtime_handle.clone(),
        admin_service,
        Arc::clone(&vault_service),
        user_id,
    );
    let users_content = users_dialog
        .take_content()
        .unwrap_or_else(|| gtk4::Box::new(Orientation::Vertical, 0).upcast::<gtk4::Widget>());
    install_inline_page(
        &users,
        users_content,
        main_stack,
        "users_view",
        heelonvault_core::tr!("main-user-nav").as_str(),
        administration_button,
    );

    let teams = build_inline_page("main-teams-view-title", "main-teams-view-intro");
    let teams_dialog = ManageTeamsDialog::new(
        application,
        window,
        runtime_handle,
        team_service,
        vault_service,
        user_id,
        session_master_key,
        active_vault_id,
        refresh_entries,
    );
    let teams_content = teams_dialog
        .take_content()
        .unwrap_or_else(|| gtk4::Box::new(Orientation::Vertical, 0).upcast::<gtk4::Widget>());
    install_inline_page(
        &teams,
        teams_content,
        main_stack,
        "teams_view",
        heelonvault_core::tr!("main-teams-nav").as_str(),
        teams_button,
    );

    AdminPages { users, teams }
}

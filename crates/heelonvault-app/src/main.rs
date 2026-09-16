#![allow(clippy::items_after_test_module, clippy::type_complexity)]
#![windows_subsystem = "windows"]

mod ui;

use std::cell::{Cell, RefCell};
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use directories::ProjectDirs;
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use secrecy::{ExposeSecret, SecretBox, SecretString};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, SqlitePool};
use tokio::runtime::Builder;
use tracing::debug;
use tracing::info;
use tracing::warn;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::prelude::*;

use crate::ui::dialogs::login_dialog::{
    AuthenticatedSession, BootstrapServicesContext, LoginDialog,
};
use crate::ui::dialogs::pin_unlock_dialog::{
    PIN_HARD_TIMEOUT as DIALOG_PIN_HARD_TIMEOUT, PinUnlockDialog,
};
use crate::ui::windows::main_window::MainWindow;
use heelonvault_core::config::constants::APP_ID;
use heelonvault_core::errors::AppError;
use heelonvault_core::models::UserRole;
#[cfg(feature = "premium")]
use heelonvault_core::repositories::audit_log_repository::SqlxAuditLogRepository;
use heelonvault_core::repositories::secret_repository::SqlxSecretRepository;
use heelonvault_core::repositories::team_repository::SqlxTeamRepository;
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::SqlxVaultRepository;
#[cfg(not(feature = "premium"))]
use heelonvault_core::services::admin_service::CommunityAdminService;
use heelonvault_core::services::admin_service::bootstrap_first_admin_with_recovery;
#[cfg(not(feature = "premium"))]
use heelonvault_core::services::audit_log_service::NoOpAuditLogService;
#[cfg(feature = "premium")]
#[cfg(feature = "premium")]
use heelonvault_core::services::audit_service::AuditAction;
#[cfg(feature = "premium")]
use heelonvault_core::services::audit_service::AuditService;
use heelonvault_core::services::auth_policy_service::{AuthPolicyService, SqlxAuthPolicyService};
use heelonvault_core::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_core::services::backup_application_service::BackupApplicationServiceImpl;
use heelonvault_core::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_core::services::crypto_service::CryptoServiceImpl;
#[cfg(not(feature = "premium"))]
use heelonvault_core::services::federated_auth_service::CommunityFederatedAuthService;
use heelonvault_core::services::import_service::ImportServiceImpl;
use heelonvault_core::services::login_history_service::record_successful_login;
use heelonvault_core::services::password_service::PasswordServiceImpl;
use heelonvault_core::services::recovery_service::reset_master_password_from_backup;
use heelonvault_core::services::rekey_service::RekeyReport;
use heelonvault_core::services::secret_service::SecretServiceImpl;
#[cfg(not(feature = "premium"))]
use heelonvault_core::services::team_service::CommunityTeamService;
use heelonvault_core::services::totp_service::SqliteTotpService;
use heelonvault_core::services::user_service::{UserService, UserServiceImpl};
use heelonvault_core::services::vault_service::VaultServiceImpl;
use heelonvault_core::utils::private_fs;
#[cfg(feature = "premium")]
use heelonvault_premium::services::admin_service_impl::AdminServiceImpl;
#[cfg(feature = "premium")]
use heelonvault_premium::services::audit_log_service_impl::AuditLogServiceImpl;
#[cfg(feature = "premium")]
use heelonvault_premium::services::license_service::LicenseService;
#[cfg(feature = "premium")]
use heelonvault_premium::services::psc_auth_service_impl::PscAuthServiceImpl;
#[cfg(feature = "premium")]
use heelonvault_premium::services::psc_config::PscConfig;
#[cfg(feature = "premium")]
use heelonvault_premium::services::team_service_impl::TeamServiceImpl;
use uuid::Uuid;

type VaultServiceHandle = VaultServiceImpl<
    SqlxVaultRepository,
    SqlxVaultRepository,
    SqlxUserRepository,
    SqlxTeamRepository,
    AuditLogServiceHandle,
    CryptoServiceImpl,
>;
type SecretServiceHandle =
    SecretServiceImpl<SqlxSecretRepository, CryptoServiceImpl, AuditLogServiceHandle>;
type UserServiceHandle = UserServiceImpl<
    SqlxUserRepository,
    SqlxVaultRepository,
    SqlxVaultRepository,
    SqlxSecretRepository,
    AuthServiceImpl<CryptoServiceImpl>,
    CryptoServiceImpl,
>;
type TotpServiceHandle = SqliteTotpService<AuthServiceImpl<CryptoServiceImpl>, CryptoServiceImpl>;

#[cfg(not(feature = "premium"))]
type AuditLogServiceHandle = NoOpAuditLogService;
#[cfg(feature = "premium")]
type AuditLogServiceHandle = AuditLogServiceImpl<SqlxUserRepository, SqlxAuditLogRepository>;

#[cfg(not(feature = "premium"))]
type AdminServiceHandle = CommunityAdminService;
#[cfg(feature = "premium")]
type AdminServiceHandle =
    AdminServiceImpl<SqlxUserRepository, AuthServiceImpl<CryptoServiceImpl>, AuditLogServiceHandle>;

#[cfg(not(feature = "premium"))]
type TeamServiceHandle = CommunityTeamService;
#[cfg(feature = "premium")]
type TeamServiceHandle = TeamServiceImpl<
    SqlxTeamRepository,
    SqlxUserRepository,
    SqlxVaultRepository,
    CryptoServiceImpl,
    AuditLogServiceHandle,
>;

type BackupApplicationServiceHandle =
    BackupApplicationServiceImpl<SqlxUserRepository, BackupServiceImpl>;

#[cfg(not(feature = "premium"))]
type FederatedAuthServiceHandle = CommunityFederatedAuthService;
#[cfg(feature = "premium")]
type FederatedAuthServiceHandle = PscAuthServiceImpl;

struct AppContext {
    database_path: PathBuf,
    pool: SqlitePool,
    crypto_service: CryptoServiceImpl,
    auth_service: Arc<AuthServiceImpl<CryptoServiceImpl>>,
    auth_policy_service: Arc<SqlxAuthPolicyService>,
    vault_service: Arc<VaultServiceHandle>,
    secret_service: Arc<SecretServiceHandle>,
    backup_service: Arc<BackupServiceImpl>,
    import_service: Arc<ImportServiceImpl>,
    user_service: Arc<UserServiceHandle>,
    totp_service: Arc<TotpServiceHandle>,
    federated_auth_service: Arc<FederatedAuthServiceHandle>,
    _audit_log_service: Arc<AuditLogServiceHandle>,
    admin_service: Arc<AdminServiceHandle>,
    team_service: Arc<TeamServiceHandle>,
    _backup_app_service: Arc<BackupApplicationServiceHandle>,
    #[cfg(feature = "premium")]
    _license_service: Arc<LicenseService>,
    _password_service: PasswordServiceImpl,
}

enum AppStartMode {
    Ready(AppContext),
    /// Database migrated but no admin user found – the init wizard must run.
    NeedsBootstrap(AppContext),
}

struct PrimaryServices {
    crypto_service: CryptoServiceImpl,
    auth_service: Arc<AuthServiceImpl<CryptoServiceImpl>>,
    audit_log_service: Arc<AuditLogServiceHandle>,
    auth_policy_service: Arc<SqlxAuthPolicyService>,
    vault_service: Arc<VaultServiceHandle>,
    secret_service: Arc<SecretServiceHandle>,
    user_service: Arc<UserServiceHandle>,
    federated_auth_service: Arc<FederatedAuthServiceHandle>,
    admin_service: Arc<AdminServiceHandle>,
    team_service: Arc<TeamServiceHandle>,
}

struct SecondaryServices {
    password_service: PasswordServiceImpl,
    backup_service: Arc<BackupServiceImpl>,
    backup_app_service: Arc<BackupApplicationServiceHandle>,
    import_service: Arc<ImportServiceImpl>,
    totp_service: Arc<TotpServiceHandle>,
    #[cfg(feature = "premium")]
    license_service: Arc<LicenseService>,
}

struct DailyLogFileWriter {
    log_dir: PathBuf,
    base_name: String,
    current_date: String,
    file: File,
}

impl DailyLogFileWriter {
    fn new(log_dir: PathBuf, base_name: impl Into<String>) -> Result<Self> {
        let base_name = base_name.into();
        let current_date = Self::date_stamp_local();
        let file = Self::open_file(&log_dir, &base_name, &current_date)?;
        Ok(Self {
            log_dir,
            base_name,
            current_date,
            file,
        })
    }

    /// Returns the current local date as `YYYYMMDD` (no hyphens).
    fn date_stamp_local() -> String {
        Local::now().format("%Y%m%d").to_string()
    }

    fn path_for(log_dir: &std::path::Path, base_name: &str, date_stamp: &str) -> PathBuf {
        log_dir.join(format!("{base_name}_{date_stamp}.log"))
    }

    fn open_file(log_dir: &std::path::Path, base_name: &str, date_stamp: &str) -> Result<File> {
        let log_path = Self::path_for(log_dir, base_name, date_stamp);
        let file = private_fs::app_open_options(&log_path)
            .and_then(|mut options| options.create(true).append(true).open(&log_path))
            .with_context(|| format!("failed to open log file {}", log_path.display()))?;
        private_fs::tighten_app_file(&log_path)
            .with_context(|| format!("failed to restrict log file {}", log_path.display()))?;
        Ok(file)
    }

    fn rotate_if_needed(&mut self) -> std::io::Result<()> {
        let today = Self::date_stamp_local();
        if today == self.current_date {
            return Ok(());
        }

        let next_file = Self::open_file(&self.log_dir, &self.base_name, &today)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        self.file = next_file;
        self.current_date = today;
        Ok(())
    }
}

impl Write for DailyLogFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.rotate_if_needed()?;
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// Configure les variables d'environnement pour GTK4/libadwaita
/// en mode "portable" (ressources à côté de l'exécutable).
///
/// Cette fonction DOIT être appelée avant toute initialisation GTK4,
/// c'est-à-dire avant `register_resources()` qui appelle `gio::resources_register_include!`.
///
/// Sur Windows, GTK4 cherche ses ressources (thèmes, icônes, schemas, loaders)
/// dans des chemins spécifiques. En mode portable, on configure ces chemins
/// pour pointer vers le dossier d'installation de l'application.
#[allow(unsafe_code)]
fn setup_windows_resources() {
    if cfg!(target_os = "windows")
        && let Ok(exe) = env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        // L'installeur MSI place l'exécutable dans INSTALLFOLDER\bin\ alors que share\ et lib\
        // sont ses *frères*, pas ses enfants (voir crates/heelonvault-app/wix/main.wxs et le
        // staging de scripts/build-msi.ps1). Les ressources GTK/GLib doivent donc être résolues
        // depuis la racine d'installation, pas depuis le dossier de l'exécutable.
        //
        // Pointer XDG_DATA_DIRS / GSETTINGS_SCHEMA_DIR sur bin\share (inexistant) faisait
        // échouer la recherche du schéma org.gtk.gtk4.Settings.FileChooser : GLib abandonne
        // alors le processus via g_error() dès l'ouverture d'un sélecteur de fichiers (export
        // DB, import CSV, restauration au login), et le thème d'icônes sur disque devenait
        // introuvable — seules les icônes embarquées dans GTK s'affichaient.
        //
        // Renseigner XDG_DATA_DIRS désactive de surcroît le repli natif de GLib sous Windows
        // (g_win32_get_system_data_dirs, qui déduit le préfixe du dossier de la DLL chargée et
        // aurait trouvé le bon chemin) : une valeur erronée est donc pire que pas de valeur.
        //
        // En mode portable (ressources à côté de l'exécutable) il n'y a pas de dossier bin :
        // la racine des ressources reste alors le dossier de l'exécutable.
        let resource_root = match exe_dir.file_name() {
            Some(name) if name.eq_ignore_ascii_case("bin") => exe_dir.parent().unwrap_or(exe_dir),
            _ => exe_dir,
        };
        let root = resource_root.to_string_lossy();
        // Les migrations, elles, restent à côté de l'exécutable (build-msi.ps1 les stage dans
        // bin\migrations) : ces deux racines diffèrent volontairement, ne pas les réunifier.
        let exe_dir = exe_dir.to_string_lossy();

        // Racine des données GTK
        // SAFETY: These are called once at startup, single-threaded, before any GTK initialization
        unsafe {
            env::set_var("GTK_DATA_PREFIX", &*root);
            env::set_var("GTK_EXE_PREFIX", &*root);
            env::set_var("XDG_DATA_DIRS", format!("{}/share", root));

            // Schemas GSettings
            env::set_var(
                "GSETTINGS_SCHEMA_DIR",
                format!("{}/share/glib-2.0/schemas", root),
            );

            // Loaders gdk-pixbuf (scan dynamique du dossier, pas de cache)
            env::set_var(
                "GDK_PIXBUF_MODULEDIR",
                format!("{}/lib/gdk-pixbuf-2.0/2.10.0/loaders", root),
            );

            // Thème par défaut
            env::set_var("GTK_THEME", "Adwaita");

            // Migrations : permet à l'application de trouver le dossier migrations
            env::set_var(
                "HEELONVAULT_MIGRATIONS_DIR",
                format!("{}/migrations", exe_dir),
            );
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let startup_flags = StartupFlags::from_args(&args);
    if startup_flags.show_version {
        // Version will be logged below with the rest of startup info
    }

    // Renderer preference should be set by the launcher environment (AppImage,
    // package scripts, or user shell) to keep this binary free of unsafe env mutation.

    let _logging_guard = init_logging()?;
    info!("HeelonVault v{} starting", env!("CARGO_PKG_VERSION"));

    // Configure Windows-specific environment variables for portable GTK4
    // MUST be called before any GTK4 initialization (before register_resources)
    setup_windows_resources();

    register_resources()?;

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start tokio runtime")?;
    let runtime = Arc::new(runtime);
    info!("tokio runtime started");

    let app_start = runtime.block_on(initialize_app_context())?;
    let (app_context, start_needs_bootstrap) = match app_start {
        AppStartMode::Ready(ctx) => (Arc::new(ctx), false),
        AppStartMode::NeedsBootstrap(ctx) => (Arc::new(ctx), true),
    };

    if startup_flags.startup_check_only {
        info!(needs_bootstrap = start_needs_bootstrap, "startup-check: ok");
        return Ok(());
    }

    run_application(
        runtime,
        app_context,
        start_needs_bootstrap,
        startup_flags.psc_callback_artifact,
    )
}

struct StartupFlags {
    show_version: bool,
    startup_check_only: bool,
    psc_callback_artifact: Option<String>,
}

impl StartupFlags {
    fn from_args(args: &[String]) -> Self {
        Self {
            show_version: args.iter().any(|arg| arg == "--version"),
            startup_check_only: args.iter().any(|arg| arg == "--startup-check"),
            psc_callback_artifact: find_psc_artifact(args),
        }
    }
}

fn find_psc_artifact(args: &[String]) -> Option<String> {
    for arg in args {
        if let Some(value) = arg.strip_prefix("--psc-artifact=")
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_string());
        }

        if let Some(query) = arg.strip_prefix("heelonvault://auth/callback?") {
            for pair in query.split('&') {
                if let Some(value) = pair.strip_prefix("artifact=")
                    && !value.trim().is_empty()
                {
                    return Some(value.trim().to_string());
                }
            }
        }
    }

    None
}

fn run_application(
    runtime: Arc<tokio::runtime::Runtime>,
    app_context: Arc<AppContext>,
    start_needs_bootstrap: bool,
    startup_psc_artifact: Option<String>,
) -> Result<()> {
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::empty())
        .build();
    application.connect_startup(|_| {
        install_application_css();
        setup_icon_theme();
    });

    let runtime_handle = runtime.handle().clone();
    let runtime_for_activate = Arc::clone(&runtime);
    let app_context_for_activate = Arc::clone(&app_context);
    let runtime_for_shutdown = Arc::clone(&runtime);
    let app_context_for_shutdown = Arc::clone(&app_context);
    let needs_bootstrap_for_activate = Rc::new(Cell::new(start_needs_bootstrap));
    let startup_psc_artifact_for_activate = startup_psc_artifact.clone();
    application.connect_activate(move |app| {
        let context = Arc::clone(&app_context_for_activate);
        let runtime_for_restore = Arc::clone(&runtime_for_activate);
        let needs_bootstrap_flag = Rc::clone(&needs_bootstrap_for_activate);
        let app_for_login = app.clone();
        let app_for_restore = app.clone();
        let login_parent = adw::ApplicationWindow::builder()
            .application(app)
            .title("HeelonVault")
            .default_width(1)
            .default_height(1)
            .build();
        login_parent.set_visible(false);
        let runtime_for_login = runtime_handle.clone();
        let context_for_login = Arc::clone(&context);
        let active_main_window: Rc<RefCell<Option<Rc<MainWindow>>>> = Rc::new(RefCell::new(None));
        let present_login_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        let active_main_for_login = Rc::clone(&active_main_window);
        let present_holder_for_login = Rc::clone(&present_login_holder);
        let needs_bootstrap_for_login = Rc::clone(&needs_bootstrap_flag);
        let startup_psc_artifact_for_login = startup_psc_artifact_for_activate.clone();
        let present_login: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(main) = active_main_for_login.borrow().as_ref() {
                main.deactivate_auto_lock();
                main.window().set_visible(false);
            }

            let app_for_cancel = app_for_login.clone();
            let app_for_restore_completed = app_for_restore.clone();
            let context_for_success = Arc::clone(&context_for_login);
            let context_for_restore = Arc::clone(&context_for_login);
            let runtime_for_success = runtime_for_login.clone();
            let runtime_for_restore_task = Arc::clone(&runtime_for_restore);
            let active_main_for_success = Rc::clone(&active_main_for_login);
            let present_holder_for_logout = Rc::clone(&present_holder_for_login);
            let needs_bootstrap_for_dialog = Rc::clone(&needs_bootstrap_for_login);
            let app_for_main_success = app_for_login.clone();
            let login_parent_for_dialog = login_parent.clone();
            let startup_psc_artifact_for_dialog = startup_psc_artifact_for_login.clone();
            let bootstrap_ctx_for_dialog = if needs_bootstrap_for_dialog.get() {
                let context_for_bootstrap = Arc::clone(&context_for_login);
                let backup_for_bootstrap = Arc::clone(&context_for_bootstrap.backup_service);
                let pool_for_bootstrap = context_for_bootstrap.pool.clone();
                let auth_for_bootstrap = Arc::clone(&context_for_bootstrap.auth_service);
                let runtime_for_bootstrap = runtime_for_login.clone();

                let context_for_bootstrap_fn = Arc::clone(&context_for_bootstrap);
                let backup_for_bootstrap_fn = Arc::clone(&backup_for_bootstrap);
                let pool_for_bootstrap_fn = pool_for_bootstrap.clone();
                let auth_for_bootstrap_fn = Arc::clone(&auth_for_bootstrap);
                let runtime_for_bootstrap_fn = runtime_for_bootstrap.clone();

                // Single source of truth: the phrase displayed at the identity step is the
                // one persisted at the oath step. Never generate a second one.
                let recovery_phrase_state: Arc<std::sync::RwLock<Option<SecretString>>> =
                    Arc::new(std::sync::RwLock::new(None));
                let recovery_state_for_gen = Arc::clone(&recovery_phrase_state);
                let recovery_state_for_bootstrap = recovery_phrase_state;

                Some(BootstrapServicesContext {
                    generate_recovery_key: Arc::new(move || {
                        let backup_service = Arc::clone(&backup_for_bootstrap);
                        let bundle = backup_service.generate_recovery_key()?;
                        let phrase = bundle.recovery_phrase.expose_secret().to_string();
                        let mut slot = recovery_state_for_gen
                            .write()
                            .map_err(|_| heelonvault_core::errors::AppError::Internal)?;
                        *slot = Some(bundle.recovery_phrase);
                        Ok(phrase)
                    }),
                    do_bootstrap: Arc::new(move |username: String, password_bytes: Vec<u8>| {
                        let recovery_phrase = recovery_state_for_bootstrap
                            .write()
                            .map_err(|_| heelonvault_core::errors::AppError::Internal)?
                            .take()
                            .ok_or_else(|| {
                                heelonvault_core::errors::AppError::Validation(
                                    "recovery phrase missing from bootstrap state".to_string(),
                                )
                            })?;

                        runtime_for_bootstrap_fn.block_on(async {
                            bootstrap_first_admin_with_recovery(
                                &SqlxUserRepository::new(pool_for_bootstrap_fn.clone()),
                                auth_for_bootstrap_fn.as_ref(),
                                backup_for_bootstrap_fn.as_ref(),
                                &context_for_bootstrap_fn.crypto_service,
                                username.as_str(),
                                SecretBox::new(Box::new(password_bytes)),
                                recovery_phrase,
                            )
                            .await
                        })
                    }),
                })
            } else {
                None
            };
            #[cfg(feature = "premium")]
            let login_license_badge_text = context_for_login
                ._license_service
                .get_cached()
                .map(|license| match license.tier {
                    heelonvault_core::models::LicenseTier::Community => "Licence free".to_string(),
                    heelonvault_core::models::LicenseTier::Professional => {
                        format!("Licence pro - {}", license.customer_name)
                    }
                })
                .unwrap_or_else(|| "Licence free".to_string());
            #[cfg(not(feature = "premium"))]
            let login_license_badge_text = "Licence free".to_string();
            let login_dialog = LoginDialog::new(
                &app_for_login,
                &login_parent_for_dialog,
                runtime_for_login.clone(),
                Arc::clone(&context_for_login.auth_service),
                Arc::clone(&context_for_login.auth_policy_service),
                Arc::clone(&context_for_login.user_service),
                Arc::clone(&context_for_login.totp_service),
                Arc::clone(&context_for_login.federated_auth_service),
                startup_psc_artifact_for_dialog,
                bootstrap_ctx_for_dialog,
                login_license_badge_text,
                move |backup_file_path, recovery_phrase, new_password| {
                    let staging_path =
                        build_restore_staging_path(&context_for_restore.database_path);
                    cleanup_restore_staging_path(&staging_path).map_err(|error| {
                        AppError::Storage(format!(
                            "failed to prepare restore staging area: {error}"
                        ))
                    })?;

                    let recovery_phrase = SecretString::new(recovery_phrase.into_boxed_str());
                    context_for_restore
                        .backup_service
                        .import_hvb_with_recovery_key(
                            backup_file_path.as_path(),
                            &recovery_phrase,
                            staging_path.as_path(),
                        )?;

                    let new_password = SecretString::new(new_password.into_boxed_str());
                    let report =
                        runtime_for_restore_task.block_on(reset_restored_database_password(
                            staging_path.as_path(),
                            &context_for_restore.crypto_service,
                            &recovery_phrase,
                            &new_password,
                        ))?;

                    runtime_for_restore_task.block_on(async {
                        context_for_restore.pool.close().await;
                    });

                    promote_staged_restore(
                        staging_path.as_path(),
                        context_for_restore.database_path.as_path(),
                    )
                    .map_err(|error| {
                        AppError::Storage(format!("failed to promote restored database: {error}"))
                    })?;
                    context_for_restore.auth_service.signal_shutdown();
                    Ok(report)
                },
                move || {
                    if let Err(error) = restart_current_process() {
                        warn!(error = %error, "failed to restart application after restore");
                    }
                    app_for_restore_completed.quit();
                },
                move |session: AuthenticatedSession| {
                    needs_bootstrap_for_dialog.set(false);
                    let login_success_started = Instant::now();
                    info!("login flow trace: authenticated callback entered");
                    let session_user_id = session.user_id;
                    let session_username = session.username.clone();
                    let session_identity_label = session.identity_label.clone();
                    let session_master_key = session.master_key.expose_secret().clone();
                    debug!(
                        key_len = session_master_key.len(),
                        "session master key derived on login"
                    );

                    let profile_load_started = Instant::now();
                    let user_profile = runtime_for_success.block_on(async {
                        context_for_success
                            .user_service
                            .get_user_profile(session_user_id)
                            .await
                            .ok()
                    });
                    info!(
                        elapsed_ms = profile_load_started.elapsed().as_millis() as u64,
                        total_elapsed_ms = login_success_started.elapsed().as_millis() as u64,
                        "login flow trace: post-login profile resolved"
                    );
                    let is_admin = user_profile
                        .as_ref()
                        .map(|u| matches!(u.role, heelonvault_core::models::UserRole::Admin))
                        .unwrap_or(false);
                    if let Some(language) =
                        user_profile.as_ref().map(|u| u.preferred_language.clone())
                    {
                        let _ = heelonvault_core::i18n::set_language(language.as_str());
                    }

                    let main_window_build_started = Instant::now();
                    #[cfg(feature = "premium")]
                    let license_badge_text = context_for_success
                        ._license_service
                        .get_cached()
                        .map(|license| match license.tier {
                            heelonvault_core::models::LicenseTier::Community => {
                                "Licence free".to_string()
                            }
                            heelonvault_core::models::LicenseTier::Professional => {
                                format!("Licence pro - {}", license.customer_name)
                            }
                        })
                        .unwrap_or_else(|| "Licence free".to_string());
                    #[cfg(not(feature = "premium"))]
                    let license_badge_text = "Licence free".to_string();
                    let main_for_success = Rc::new(MainWindow::new(
                        &app_for_main_success,
                        runtime_for_success.clone(),
                        Arc::clone(&context_for_success.secret_service),
                        Arc::clone(&context_for_success.vault_service),
                        Arc::clone(&context_for_success.user_service),
                        Arc::clone(&context_for_success.admin_service),
                        Arc::clone(&context_for_success.team_service),
                        Arc::clone(&context_for_success.totp_service),
                        Arc::clone(&context_for_success.auth_policy_service),
                        Arc::clone(&context_for_success.backup_service),
                        Arc::clone(&context_for_success._backup_app_service),
                        Arc::clone(&context_for_success.import_service),
                        #[cfg(feature = "premium")]
                        Arc::clone(&context_for_success._license_service),
                        context_for_success.pool.clone(),
                        context_for_success.database_path.clone(),
                        session_user_id,
                        session_master_key,
                        session_identity_label,
                        license_badge_text,
                        is_admin,
                    ));
                    info!(
                        elapsed_ms = main_window_build_started.elapsed().as_millis() as u64,
                        total_elapsed_ms = login_success_started.elapsed().as_millis() as u64,
                        "login flow trace: MainWindow::new completed"
                    );
                    main_for_success.window().set_icon_name(Some("heelonvault"));
                    // Afficher la fenêtre principale AVANT de fermer la fenêtre de login
                    // Sinon GTK termine l'application quand la dernière fenêtre est fermée
                    main_for_success.window().present();

                    let refresh_entries_started = Instant::now();
                    main_for_success.refresh_entries();
                    info!(
                        elapsed_ms = refresh_entries_started.elapsed().as_millis() as u64,
                        total_elapsed_ms = login_success_started.elapsed().as_millis() as u64,
                        "login flow trace: refresh_entries invoked"
                    );

                    // A migrated account whose recovery phrase could not be read at migration
                    // time cannot be restored from a backup until the phrase is sealed again.
                    let (recovery_sender, recovery_receiver) = tokio::sync::oneshot::channel();
                    let runtime_for_recovery = runtime_for_success.clone();
                    let user_service_for_recovery = Arc::clone(&context_for_success.user_service);
                    std::thread::spawn(move || {
                        let result = runtime_for_recovery.block_on(async move {
                            user_service_for_recovery
                                .account_needs_recovery_key(session_user_id)
                                .await
                        });
                        let _ = recovery_sender.send(result);
                    });
                    let main_for_recovery = Rc::clone(&main_for_success);
                    glib::MainContext::default().spawn_local(async move {
                        if let Ok(Ok(true)) = recovery_receiver.await {
                            let dialog = adw::MessageDialog::new(
                                Some(main_for_recovery.window()),
                                Some(heelonvault_core::tr!("recovery-key-missing-title").as_str()),
                                Some(heelonvault_core::tr!("recovery-key-missing-body").as_str()),
                            );
                            dialog.add_response("ok", heelonvault_core::tr!("common-ok").as_str());
                            dialog.set_default_response(Some("ok"));
                            dialog.set_close_response("ok");
                            dialog.present();
                        }
                    });

                    let runtime_for_history = runtime_for_success.clone();
                    let pool_for_history = context_for_success.pool.clone();
                    let user_id_for_history = session_user_id;
                    std::thread::spawn(move || {
                        let device_info = format!("{} / GTK4 Desktop", std::env::consts::OS);
                        runtime_for_history.block_on(async move {
                            let _ = record_successful_login(
                                &pool_for_history,
                                user_id_for_history,
                                None,
                                Some(device_info.as_str()),
                            )
                            .await;
                        });
                    });

                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    let runtime_for_task = runtime_for_success.clone();
                    let policy_for_task = Arc::clone(&context_for_success.auth_policy_service);
                    let username_for_task = session_username;
                    std::thread::spawn(move || {
                        let result = runtime_for_task.block_on(async move {
                            policy_for_task
                                .get_auto_lock_delay(username_for_task.as_str())
                                .await
                        });
                        let _ = sender.send(result);
                    });

                    let main_for_delay = Rc::clone(&main_for_success);
                    glib::MainContext::default().spawn_local(async move {
                        if let Ok(Ok(delay_mins)) = receiver.await {
                            main_for_delay.set_auto_lock_timeout(delay_mins as u64);
                        }
                    });

                    let main_for_logout = Rc::clone(&main_for_success);
                    let active_main_for_logout = Rc::clone(&active_main_for_success);
                    let present_for_logout = Rc::clone(&present_holder_for_logout);
                    main_for_success.set_on_logout(Rc::new(move || {
                        let user_id = main_for_logout.session_user_id();
                        if main_for_logout.has_pin_cache(user_id, DIALOG_PIN_HARD_TIMEOUT) {
                            // PIN cache valide : verrouiller sans vider le cache,
                            // puis afficher la dialog PIN (même chemin que l'auto-lock).
                            info!("logout intercepted by PIN cache — showing PIN unlock dialog");
                            main_for_logout.lock_session_keep_pin();
                            main_for_logout.trigger_pin_lock();
                        } else {
                            // Pas de PIN cache : déconnexion complète.
                            info!("main window logout requested, clearing sensitive session and returning to login");
                            main_for_logout.clear_sensitive_session();
                            main_for_logout.window().set_visible(false);
                            *active_main_for_logout.borrow_mut() = None;
                            info!("main window logout completed, login screen will be presented again");
                            if let Some(present_login_cb) = present_for_logout.borrow().as_ref() {
                                present_login_cb.as_ref()();
                            }
                        }
                    }));

                    let main_for_auto_lock = Rc::clone(&main_for_success);
                    main_for_success.set_on_auto_lock(Rc::new(move || {
                        // If a valid PIN cache exists for this user, show the PIN unlock
                        // dialog instead of forcing a full master-password re-entry.
                        let user_id = main_for_auto_lock.session_user_id();
                        if main_for_auto_lock.has_pin_cache(user_id, DIALOG_PIN_HARD_TIMEOUT) {
                            main_for_auto_lock.trigger_pin_lock();
                        } else {
                            main_for_auto_lock.trigger_logout();
                        }
                    }));

                    // PIN lock callback: shows the PinUnlockDialog.
                    // On success the master key is restored and the auto-lock timer restarted.
                    // On fallback ("use master password") we trigger a full logout + re-login.
                    let main_for_pin_lock = Rc::clone(&main_for_success);
                    let present_for_pin_fallback = Rc::clone(&present_holder_for_logout);
                    main_for_success.set_on_pin_lock(Rc::new(move || {
                        let main_inner = Rc::clone(&main_for_pin_lock);
                        let main_for_on_unlocked = Rc::clone(&main_for_pin_lock);
                        let main_for_fallback = Rc::clone(&main_for_pin_lock);
                        let present_fallback = Rc::clone(&present_for_pin_fallback);
                        let dialog = PinUnlockDialog::new(
                            main_inner.window(),
                            Rc::clone(&main_inner),
                            move |master_key| {
                                let Some(key) = master_key else {
                                    // Exhausted — fall back to full login.
                                    main_for_on_unlocked.trigger_logout();
                                    return;
                                };
                                main_for_on_unlocked.set_session_master_key(key.to_vec());
                                main_for_on_unlocked.activate_auto_lock();
                                info!("PIN unlock successful — session restored");
                            },
                            move || {
                                // User chose "use master password".
                                main_for_fallback.clear_pin_cache();
                                main_for_fallback.trigger_logout();
                                if let Some(present_login_cb) = present_fallback.borrow().as_ref() {
                                    present_login_cb.as_ref()();
                                }
                            },
                        );
                        dialog.present();
                    }));

                    *active_main_for_success.borrow_mut() = Some(Rc::clone(&main_for_success));

                    let present_started = Instant::now();
                    main_for_success.window().present();
                    info!(
                        elapsed_ms = present_started.elapsed().as_millis() as u64,
                        total_elapsed_ms = login_success_started.elapsed().as_millis() as u64,
                        "login flow trace: main window present() called"
                    );
                    let login_success_started_for_idle = login_success_started;
                    glib::idle_add_local_once(move || {
                        info!(
                            total_elapsed_ms =
                                login_success_started_for_idle.elapsed().as_millis() as u64,
                            "login flow trace: main loop reached first idle after present"
                        );
                    });
                    main_for_success.activate_auto_lock();
                },
                move || {
                    app_for_cancel.quit();
                },
            );
            login_dialog.present();
        });

        *present_login_holder.borrow_mut() = Some(Rc::clone(&present_login));

        present_login.as_ref()();
    });

    application.connect_shutdown(move |_| {
        info!("application shutdown requested, closing services");
        ui::sensitive_clipboard::clear_now();
        app_context_for_shutdown.auth_service.signal_shutdown();

        let pool = app_context_for_shutdown.pool.clone();
        runtime_for_shutdown.block_on(async move {
            pool.close().await;
        });

        info!("application shutdown completed");
    });

    let _exit_code = application.run();
    Ok(())
}

fn register_resources() -> Result<()> {
    gio::resources_register_include!("heelonvault.gresource")
        .context("failed to register compiled resources")?;
    Ok(())
}

fn setup_icon_theme() {
    gtk4::Window::set_default_icon_name("heelonvault");
    if let Some(display) = gdk::Display::default() {
        let theme = gtk4::IconTheme::for_display(&display);
        theme.add_resource_path("/com/heelonvault/rust");
    }
}

fn install_application_css() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_resource("/com/heelonvault/rust/style.css");

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Custom timer for `tracing_subscriber` that formats each log record timestamp
/// as RFC 3339 in the system's local timezone (e.g. `2026-03-17T14:40:06+01:00`).
/// Using `chrono::Local` avoids the `SAFETY` caveats of `time::UtcOffset::current_local_offset()`
/// in a multi-threaded context.
struct LocalRfc3339Timer;

impl FormatTime for LocalRfc3339Timer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().to_rfc3339())
    }
}

fn init_logging() -> Result<WorkerGuard> {
    const SENSITIVE_TARGETS: &[&str] = &[
        "vault::crypto",
        "auth::session",
        "heelonvault_core::services::crypto_service",
        "heelonvault_core::services::secret_service",
        "heelonvault_core::services::auth_service",
        "heelonvault_core::services::vault_service",
    ];

    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };
    let base_filter_spec = env::var("RUST_LOG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("HEELONVAULT_LOG_LEVEL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| default_level.to_string());

    let mut filter_spec = base_filter_spec.clone();
    for target in SENSITIVE_TARGETS {
        if !base_filter_spec.contains(target) {
            filter_spec.push(',');
            filter_spec.push_str(target);
            filter_spec.push_str("=warn");
        }
    }

    let env_filter = EnvFilter::try_new(filter_spec.clone())
        .with_context(|| format!("invalid log level/filter: {filter_spec}"))?;

    let log_dir_path = if let Some(path_raw) = env::var("HEELONVAULT_LOG_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        PathBuf::from(path_raw)
    } else {
        let current_dir = env::current_dir().context("failed to resolve current directory")?;
        resolve_default_log_dir(&current_dir)
    };
    private_fs::create_private_dir_all(&log_dir_path)
        .with_context(|| format!("failed to create log directory {}", log_dir_path.display()))?;

    let rolling_writer = DailyLogFileWriter::new(log_dir_path.clone(), "heelonvault")?;
    let (file_writer, guard) = tracing_appender::non_blocking(rolling_writer);

    let is_debug_logging = base_filter_spec.to_ascii_lowercase().contains("debug");
    let is_dev_mode = cfg!(debug_assertions);

    // Check if console output is explicitly requested via environment variable
    let force_console = env::var("HEELONVAULT_CONSOLE")
        .ok()
        .filter(|v| v == "1" || v.to_lowercase() == "true")
        .is_some();

    // File layer is always active - writes JSON logs to files
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false)
        .with_writer(file_writer)
        .with_target(is_debug_logging)
        .with_line_number(is_debug_logging)
        .with_file(is_debug_logging)
        .with_timer(LocalRfc3339Timer);

    // On Windows with GUI subsystem, console output is not visible.
    // Console layer is only added on non-Windows platforms.
    // Note: On Windows, even with HEELONVAULT_CONSOLE=1, the subsystem is GUI so stdout won't show.
    if (is_dev_mode || force_console) && !cfg!(windows) {
        // Non-Windows: add console layer (default formatter)
        let console_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout)
            .with_target(false)
            .with_timer(LocalRfc3339Timer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(console_layer)
            .try_init()
            .map_err(|error| anyhow!("failed to initialize tracing subscriber: {error}"))?;
    } else {
        // Windows or console not requested: only file layer
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .try_init()
            .map_err(|error| anyhow!("failed to initialize tracing subscriber: {error}"))?;
    }

    // Set a panic hook that routes to tracing::error! to avoid silent panics
    // Critical now that Windows console is hidden by GUI subsystem
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map(|l| l.to_string()).unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        tracing::error!(%location, %message, "application panicked");

        // Le layer fichier passe par tracing_appender::non_blocking : son tampon est vidé par
        // un thread de fond, et il est perdu si le processus meurt immédiatement — c'est
        // typiquement le cas d'un panic Rust traversant une frontière FFI GTK, qui se termine
        // en abort(). Un panic pouvait donc ne laisser AUCUNE trace, et l'absence de log ne
        // prouvait pas l'absence de panic.
        //
        // stderr est donc écrit ici, synchronement. Invisible au double-clic sous Windows
        // (sous-système GUI, aucune console attachée), mais capturable en lançant
        // l'exécutable avec `2> fichier.log` : souvent le seul moyen de voir un panic sur un
        // build Windows packagé.
        use std::io::Write as _;
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "application panicked at {location}: {message}");
        let _ = stderr.flush();
    }));

    harden_log_dir(&log_dir_path);
    Ok(guard)
}

fn build_primary_services(pool: &SqlitePool) -> PrimaryServices {
    let crypto_service = CryptoServiceImpl::default();
    let auth_service = Arc::new(AuthServiceImpl::new(CryptoServiceImpl::default()));
    #[cfg(not(feature = "premium"))]
    let audit_log_service = Arc::new(NoOpAuditLogService);
    #[cfg(feature = "premium")]
    let audit_log_service = Arc::new(AuditLogServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        SqlxAuditLogRepository::new(pool.clone()),
    ));
    let auth_policy_service = Arc::new(SqlxAuthPolicyService::new(pool.clone()));
    let vault_service = Arc::new(VaultServiceImpl::new(
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxUserRepository::new(pool.clone()),
        SqlxTeamRepository::new(pool.clone()),
        Arc::clone(&audit_log_service),
        CryptoServiceImpl::default(),
    ));
    let secret_service = Arc::new(SecretServiceImpl::new(
        SqlxSecretRepository::new(pool.clone()),
        CryptoServiceImpl::default(),
        Arc::clone(&audit_log_service),
    ));
    let user_service = Arc::new(UserServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxSecretRepository::new(pool.clone()),
        Arc::clone(&auth_service),
        CryptoServiceImpl::default(),
    ));
    #[cfg(not(feature = "premium"))]
    let federated_auth_service = Arc::new(CommunityFederatedAuthService);
    #[cfg(feature = "premium")]
    let federated_auth_service = Arc::new(
        PscConfig::from_env()
            .map(PscAuthServiceImpl::with_config)
            .unwrap_or_else(|error| {
                warn!(
                    error = %error,
                    "PSC configuration not available; using non-operational premium placeholder"
                );
                PscAuthServiceImpl::with_config(PscConfig {
                    client_id: "psc-missing-client-id".to_string(),
                    client_secret: "psc-missing-client-secret".to_string(),
                    authorization_url: "https://auth.bas.psc.esante.gouv.fr/disabled".to_string(),
                    token_url: "https://auth.bas.psc.esante.gouv.fr/disabled".to_string(),
                    userinfo_url: "https://auth.bas.psc.esante.gouv.fr/disabled".to_string(),
                    callback_exchange_url: "https://sandbox.heelonys.fr/oidc/artefact".to_string(),
                    redirect_uri: "https://sandbox.heelonys.fr/oidc/callback/default".to_string(),
                    scope: "openid profile rpps".to_string(),
                })
            }),
    );
    #[cfg(not(feature = "premium"))]
    let admin_service = Arc::new(CommunityAdminService);
    #[cfg(feature = "premium")]
    let admin_service = Arc::new(AdminServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        Arc::clone(&auth_service),
        Arc::clone(&audit_log_service),
    ));
    #[cfg(not(feature = "premium"))]
    let team_service = Arc::new(CommunityTeamService);
    #[cfg(feature = "premium")]
    let team_service = Arc::new(TeamServiceImpl::new(
        SqlxTeamRepository::new(pool.clone()),
        SqlxUserRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        CryptoServiceImpl::default(),
        Arc::clone(&audit_log_service),
    ));

    PrimaryServices {
        crypto_service,
        auth_service,
        audit_log_service,
        auth_policy_service,
        vault_service,
        secret_service,
        user_service,
        federated_auth_service,
        admin_service,
        team_service,
    }
}

async fn build_secondary_services(
    pool: &SqlitePool,
    auth_service: Arc<AuthServiceImpl<CryptoServiceImpl>>,
) -> SecondaryServices {
    let password_service = PasswordServiceImpl::new();
    let backup_service = Arc::new(BackupServiceImpl::new());
    let backup_app_service = Arc::new(BackupApplicationServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        BackupServiceImpl::new(),
        pool.clone(),
    ));
    let import_service = Arc::new(ImportServiceImpl::new());
    let totp_service = Arc::new(SqliteTotpService::new(
        pool.clone(),
        Arc::clone(&auth_service),
        CryptoServiceImpl::default(),
        "HeelonVault",
    ));
    #[cfg(feature = "premium")]
    let license_service = {
        let audit_service = Arc::new(AuditService::new(pool.clone()));
        let mut ls = LicenseService::new();
        match ls.load_license().await {
            Ok(license) => {
                info!(customer = license.customer_name, tier = %license.tier, "license loaded successfully");
                audit_service.log_async(
                    None,
                    AuditAction::LicenseCheckSuccess,
                    Some("license"),
                    None,
                    Some(&format!("{}({})", license.tier, license.customer_name)),
                );
            }
            Err(e) => {
                warn!(error = %e, "failed to load license, defaulting to community edition");
                audit_service.log_async(
                    None,
                    AuditAction::LicenseCheckFailure,
                    Some("license"),
                    None,
                    Some(&format!("license verification failed: {}", e)),
                );
            }
        }
        Arc::new(ls)
    };

    SecondaryServices {
        password_service,
        backup_service,
        backup_app_service,
        import_service,
        totp_service,
        #[cfg(feature = "premium")]
        license_service,
    }
}

async fn initialize_app_context() -> Result<AppStartMode> {
    let database_path = resolve_database_path()?;
    prepare_database_files(&database_path)?;

    let connect_options = hardened_sqlite_options(&database_path).create_if_missing(true);

    let pool = SqlitePool::connect_with(connect_options)
        .await
        .with_context(|| {
            format!(
                "failed to open sqlite database at {}",
                database_path.display()
            )
        })?;

    // Resolve migrations path robustly across dev, bundle and installed layouts.
    let migrations_path = resolve_migrations_path()?;
    info!(path = %migrations_path.display(), "migrations path resolved");
    sqlx::migrate::Migrator::new(migrations_path.as_path())
        .await
        .context("failed to load sqlx migrations")?
        .run(&pool)
        .await
        .context("failed to run sqlx migrations")?;
    info!(database = %database_path.display(), "sqlx migrations applied successfully");

    let primary = build_primary_services(&pool);

    let needs_bootstrap =
        match ensure_privileged_account_context_initialized(&pool, &primary.auth_service).await {
            Ok(()) => false,
            Err(e)
                if e.downcast_ref::<AppError>()
                    .is_some_and(|ae| matches!(ae, AppError::InitializationRequired(_))) =>
            {
                true
            }
            Err(e) => return Err(e),
        };

    if !needs_bootstrap {
        load_password_envelopes_from_db(
            &SqlxUserRepository::new(pool.clone()),
            &primary.auth_service,
        )
        .await?;
    }

    let secondary = build_secondary_services(&pool, Arc::clone(&primary.auth_service)).await;

    info!("all services are initialized and ready");

    let ctx = AppContext {
        database_path,
        pool,
        crypto_service: primary.crypto_service,
        auth_service: primary.auth_service,
        auth_policy_service: primary.auth_policy_service,
        vault_service: primary.vault_service,
        secret_service: primary.secret_service,
        backup_service: secondary.backup_service,
        import_service: secondary.import_service,
        user_service: primary.user_service,
        totp_service: secondary.totp_service,
        federated_auth_service: primary.federated_auth_service,
        _audit_log_service: primary.audit_log_service,
        admin_service: primary.admin_service,
        team_service: primary.team_service,
        _backup_app_service: secondary.backup_app_service,
        #[cfg(feature = "premium")]
        _license_service: secondary.license_service,
        _password_service: secondary.password_service,
    };
    if needs_bootstrap {
        Ok(AppStartMode::NeedsBootstrap(ctx))
    } else {
        Ok(AppStartMode::Ready(ctx))
    }
}
#[cfg(test)]
mod tests {
    use super::{resolve_default_database_path_for, resolve_default_log_dir_for};
    use std::path::Path;

    #[test]
    fn default_database_path_uses_root_data_when_cwd_is_project_root() {
        let project_root = Path::new("/tmp/heelonvault");
        assert_eq!(
            resolve_default_database_path_for(project_root, None),
            project_root.join("data").join("heelonvault-rust-dev.db")
        );
    }

    #[test]
    fn default_database_path_uses_parent_data_when_cwd_is_rust_dir() {
        let rust_dir = Path::new("/tmp/heelonvault/rust");
        assert_eq!(
            resolve_default_database_path_for(rust_dir, None),
            Path::new("/tmp/heelonvault")
                .join("data")
                .join("heelonvault-rust-dev.db")
        );
    }

    #[test]
    fn windows_database_path_uses_local_app_data_layout() {
        let project_root = Path::new("C:/Program Files/HeelonVault");
        let runtime_root = Some(Path::new("C:/Users/test/AppData/Local/heelonvault").to_path_buf());

        assert_eq!(
            resolve_default_database_path_for(project_root, runtime_root),
            Path::new("C:/Users/test/AppData/Local/heelonvault")
                .join("data")
                .join("heelonvault-rust.db")
        );
    }

    #[test]
    fn windows_log_dir_uses_local_app_data_layout() {
        let project_root = Path::new("C:/Program Files/HeelonVault");
        let runtime_root = Some(Path::new("C:/Users/test/AppData/Local/heelonvault").to_path_buf());

        assert_eq!(
            resolve_default_log_dir_for(project_root, runtime_root),
            Path::new("C:/Users/test/AppData/Local/heelonvault").join("logs")
        );
    }

    #[test]
    fn macos_database_path_uses_app_support_layout() {
        let bundle_dir = Path::new("/Applications/HeelonVault.app/Contents/MacOS");
        let runtime_root =
            Some(Path::new("/Users/test/Library/Application Support/heelonvault").to_path_buf());

        assert_eq!(
            resolve_default_database_path_for(bundle_dir, runtime_root),
            Path::new("/Users/test/Library/Application Support/heelonvault")
                .join("data")
                .join("heelonvault-rust.db")
        );
    }

    #[test]
    fn macos_log_dir_uses_app_support_layout() {
        let bundle_dir = Path::new("/Applications/HeelonVault.app/Contents/MacOS");
        let runtime_root =
            Some(Path::new("/Users/test/Library/Application Support/heelonvault").to_path_buf());

        assert_eq!(
            resolve_default_log_dir_for(bundle_dir, runtime_root),
            Path::new("/Users/test/Library/Application Support/heelonvault").join("logs")
        );
    }
}

fn build_restore_staging_path(database_path: &Path) -> PathBuf {
    let file_name = database_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("heelonvault-rust.db");
    database_path.with_file_name(format!("{file_name}.restore.tmp"))
}

fn cleanup_restore_staging_path(staging_path: &Path) -> Result<()> {
    if staging_path.exists() {
        fs::remove_file(staging_path).with_context(|| {
            format!(
                "failed to remove previous staged restore {}",
                staging_path.display()
            )
        })?;
    }

    let staging_old_path = staging_path.with_extension("old");
    if staging_old_path.exists() {
        fs::remove_file(&staging_old_path).with_context(|| {
            format!(
                "failed to remove previous staged restore backup {}",
                staging_old_path.display()
            )
        })?;
    }

    Ok(())
}

fn promote_staged_restore(staging_path: &Path, database_path: &Path) -> Result<()> {
    if let Some(parent) = database_path.parent()
        && !parent.as_os_str().is_empty()
    {
        private_fs::create_private_dir_all(parent)
            .with_context(|| format!("failed to create database directory {}", parent.display()))?;
    }

    let old_database_path = database_path.with_extension("old");
    if old_database_path.exists() {
        fs::remove_file(&old_database_path).with_context(|| {
            format!(
                "failed to remove previous rotated database {}",
                old_database_path.display()
            )
        })?;
    }

    let original_database_was_present = database_path.exists();
    if original_database_was_present {
        fs::rename(database_path, &old_database_path).with_context(|| {
            format!(
                "failed to rotate current database to {}",
                old_database_path.display()
            )
        })?;
        // Kept as the only way back to the previous data, which includes its key material.
        private_fs::tighten_app_file(&old_database_path)
            .with_context(|| format!("failed to restrict {}", old_database_path.display()))?;
    }

    if let Err(error) = fs::rename(staging_path, database_path) {
        if original_database_was_present && old_database_path.exists() {
            let _ = fs::rename(&old_database_path, database_path);
        }
        return Err(anyhow!(
            "failed to promote staged restore {} to {}: {}",
            staging_path.display(),
            database_path.display(),
            error
        ));
    }

    Ok(())
}

fn restart_current_process() -> Result<()> {
    let current_executable = env::current_exe().context("failed to resolve current executable")?;
    let args: Vec<String> = env::args().skip(1).collect();
    Command::new(&current_executable)
        .args(args)
        .spawn()
        .with_context(|| {
            format!(
                "failed to restart application from {}",
                current_executable.display()
            )
        })?;
    Ok(())
}

async fn reset_restored_database_password(
    database_path: &Path,
    crypto_service: &CryptoServiceImpl,
    recovery_phrase: &SecretString,
    new_password: &SecretString,
) -> Result<RekeyReport, AppError> {
    let connect_options = hardened_sqlite_options(database_path).create_if_missing(false);
    let pool = SqlitePool::connect_with(connect_options)
        .await
        .map_err(|error| {
            AppError::Storage(format!(
                "failed to open restored database at {}: {error}",
                database_path.display()
            ))
        })?;

    let result = async {
        // The backup may predate the current schema.
        let migrations_path =
            resolve_migrations_path().map_err(|error| AppError::Storage(format!("{error:#}")))?;
        sqlx::migrate::Migrator::new(migrations_path.as_path())
            .await
            .map_err(|error| {
                AppError::Storage(format!(
                    "failed to load migrations for restored database: {error}"
                ))
            })?
            .run(&pool)
            .await
            .map_err(|error| {
                AppError::Storage(format!("failed to migrate restored database: {error}"))
            })?;
        reset_master_password_from_backup(&pool, crypto_service, recovery_phrase, new_password)
            .await
    }
    .await;

    pool.close().await;
    result
}

async fn ensure_privileged_account_context_initialized(
    pool: &SqlitePool,
    auth_service: &Arc<AuthServiceImpl<CryptoServiceImpl>>,
) -> Result<()> {
    let (_privileged_user_id, privileged_username, password_envelope): (
        Uuid,
        String,
        Option<Vec<u8>>,
    ) = match sqlx::query(
        "SELECT id, username, password_envelope FROM users WHERE role = ?1 ORDER BY rowid LIMIT 1",
    )
    .bind(UserRole::Admin.to_db_str())
    .fetch_optional(pool)
    .await
    .context("failed to query privileged account")?
    {
        Some(row) => {
            let id_raw: String = row
                .try_get("id")
                .context("failed to read privileged account user id")?;
            let parsed_id =
                Uuid::parse_str(&id_raw).context("failed to parse privileged account user id")?;
            let username: String = row
                .try_get("username")
                .context("failed to read privileged account username")?;
            let envelope: Option<Vec<u8>> = row
                .try_get("password_envelope")
                .context("failed to read privileged account password envelope")?;
            (parsed_id, username, envelope)
        }
        None => {
            return Err(anyhow!(AppError::InitializationRequired(
				"missing required privileged account with admin role; run the explicit initialization flow before startup"
					.to_string(),
			)));
        }
    };

    if let Some(envelope) = password_envelope {
        auth_service
            .upsert_password_envelope(
                privileged_username.as_str(),
                SecretBox::new(Box::new(envelope)),
            )
            .await
            .map_err(|error| {
                anyhow!("failed to load persisted privileged auth credentials: {error}")
            })?;
        info!(username = %privileged_username, "privileged account credentials loaded from password envelope");
    } else {
        return Err(anyhow!(AppError::InitializationRequired(
			"privileged account with admin role has no password envelope; run the explicit initialization flow before startup"
				.to_string(),
		)));
    }

    Ok(())
}

async fn load_password_envelopes_from_db(
    user_repo: &SqlxUserRepository,
    auth_service: &Arc<AuthServiceImpl<CryptoServiceImpl>>,
) -> Result<()> {
    let envelopes = user_repo
        .list_all_password_envelopes()
        .await
        .map_err(|error| anyhow!("failed to list password envelopes: {error}"))?;

    for (username, envelope) in &envelopes {
        auth_service
            .upsert_password_envelope(
                username.as_str(),
                SecretBox::new(Box::new(envelope.clone())),
            )
            .await
            .map_err(|error| anyhow!("failed to load credentials for user {username}: {error}"))?;
    }

    info!(
        count = envelopes.len(),
        "loaded password envelopes from database into auth service"
    );
    Ok(())
}

/// Freed pages are zeroed and temporary tables stay in memory, so neither replaced key
/// envelopes nor query spills linger on disk.
fn hardened_sqlite_options(database_path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(database_path)
        .pragma("secure_delete", "ON")
        .pragma("temp_store", "MEMORY")
}

/// Creates the database owner-only (SQLite would follow the umask) and restricts existing files.
fn prepare_database_files(database_path: &Path) -> Result<()> {
    let parent = match database_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    private_fs::create_private_dir_all(parent)
        .with_context(|| format!("failed to create database directory {}", parent.display()))?;

    if database_path.exists() {
        for path in database_file_set(database_path) {
            if path.exists()
                && private_fs::tighten_app_file(&path)
                    .with_context(|| format!("failed to restrict {}", path.display()))?
            {
                info!(file = %path.display(), "database file access restricted");
            }
        }
    } else {
        private_fs::app_open_options(database_path)
            .and_then(|mut options| options.write(true).create_new(true).open(database_path))
            .with_context(|| {
                format!("failed to create database file {}", database_path.display())
            })?;
    }

    warn_if_world_accessible(parent);
    warn_if_outside_user_profile(parent);
    Ok(())
}

/// The database plus the files SQLite and the restore flow keep next to it.
fn database_file_set(database_path: &Path) -> Vec<PathBuf> {
    let with_suffix = |suffix: &str| {
        let mut name = database_path.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    };
    vec![
        database_path.to_path_buf(),
        with_suffix("-wal"),
        with_suffix("-shm"),
        with_suffix("-journal"),
        database_path.with_extension("old"),
    ]
}

/// Log files can hold usernames and the raw CSV rows rejected by an import.
fn harden_log_dir(log_dir: &Path) {
    let entries = match fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) => {
            warn!(dir = %log_dir.display(), %error, "cannot inspect log directory");
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_app_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                (name.starts_with("heelonvault_") && name.ends_with(".log"))
                    || name.starts_with("csv_import_rejects_")
            });
        if is_app_file && let Err(error) = private_fs::tighten_app_file(&path) {
            warn!(file = %path.display(), %error, "cannot restrict log file permissions");
        }
    }
    warn_if_world_accessible(log_dir);
}

/// Existing directories are never changed (they may be shared on purpose): only reported.
fn warn_if_world_accessible(dir: &Path) {
    match private_fs::is_world_accessible(dir) {
        Ok(true) => warn!(
            dir = %dir.display(),
            "directory is readable by other local users; restrict it with `chmod 700`"
        ),
        Ok(false) => {}
        Err(error) => warn!(dir = %dir.display(), %error, "cannot inspect directory permissions"),
    }
}

/// On Windows the profile ACL is what keeps other users out; elsewhere it no longer applies.
fn warn_if_outside_user_profile(database_dir: &Path) {
    if !cfg!(windows) {
        return;
    }
    let Some(base_dirs) = directories::BaseDirs::new() else {
        return;
    };
    if let (Ok(dir), Ok(home)) = (
        fs::canonicalize(database_dir),
        fs::canonicalize(base_dirs.home_dir()),
    ) && !dir.starts_with(&home)
    {
        warn!(
            dir = %dir.display(),
            "database is outside the user profile; other accounts may be able to read it"
        );
    }
}

fn resolve_database_path() -> Result<PathBuf> {
    if let Ok(path_raw) = env::var("HEELONVAULT_DB_PATH") {
        let trimmed = path_raw.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let current_dir = env::current_dir().context("failed to resolve current directory")?;
    Ok(resolve_default_database_path(&current_dir))
}

fn resolve_default_database_path(current_dir: &Path) -> PathBuf {
    resolve_default_database_path_for(current_dir, resolve_platform_runtime_root())
}

fn resolve_default_database_path_for(
    current_dir: &Path,
    platform_runtime_root: Option<PathBuf>,
) -> PathBuf {
    if let Some(runtime_root) = platform_runtime_root {
        return runtime_root.join("data").join("heelonvault-rust.db");
    }

    let db_name = "heelonvault-rust-dev.db";
    if current_dir.file_name().is_some_and(|name| name == "rust")
        && let Some(project_root) = current_dir.parent()
    {
        return project_root.join("data").join(db_name);
    }

    current_dir.join("data").join(db_name)
}

fn resolve_default_log_dir(current_dir: &Path) -> PathBuf {
    resolve_default_log_dir_for(current_dir, resolve_platform_runtime_root())
}

fn resolve_migrations_path() -> Result<PathBuf> {
    // 1) Explicit override (launcher, packaging scripts)
    if let Ok(path_raw) = env::var("HEELONVAULT_MIGRATIONS_DIR") {
        let trimmed = path_raw.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }

    let exe_dir = env::current_exe()
        .context("failed to get exe path")?
        .parent()
        .context("failed to get exe dir")?
        .to_path_buf();

    let cwd = env::current_dir().context("failed to resolve current directory")?;
    let workspace_candidate = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("migrations");

    // Nouveau candidat : migrations/ dans le dossier de la crate (après restructuration)
    let crate_migrations_candidate = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");

    // 2) Installed layout: sibling of executable
    // 3) Dev layout: cwd/migrations (e.g. workspace root)
    // 4) Compile-time workspace fallback for local runs from subdirs
    // 5) Crate-local layout: migrations/ dans le dossier de la crate
    let candidates = [
        exe_dir.join("migrations"),
        cwd.join("migrations"),
        crate_migrations_candidate,
        workspace_candidate,
    ];

    for candidate in &candidates {
        if candidate.is_dir() {
            return Ok(candidate.to_path_buf());
        }
    }

    let checked = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(anyhow!(
        "no migrations directory found; checked: {checked}. You can override with HEELONVAULT_MIGRATIONS_DIR"
    ))
}

fn resolve_default_log_dir_for(
    current_dir: &Path,
    platform_runtime_root: Option<PathBuf>,
) -> PathBuf {
    if let Some(runtime_root) = platform_runtime_root {
        return runtime_root.join("logs");
    }

    current_dir.join("logs")
}

fn resolve_platform_runtime_root() -> Option<PathBuf> {
    let proj_dirs = ProjectDirs::from("fr", "Heelonys", "HeelonVault")?;

    if cfg!(target_os = "windows") {
        Some(proj_dirs.data_local_dir().join("heelonvault"))
    } else if cfg!(target_os = "macos") {
        Some(proj_dirs.data_dir().join("heelonvault"))
    } else {
        None
    }
}

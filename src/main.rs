#![allow(clippy::items_after_test_module, clippy::type_complexity)]

use std::cell::{Cell, RefCell};
use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use secrecy::{ExposeSecret, SecretBox};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, SqlitePool};
use tokio::runtime::Builder;
use tracing::debug;
use tracing::info;
use tracing::warn;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use heelonvault_rust::config::constants::APP_ID;
use heelonvault_rust::errors::AppError;
use heelonvault_rust::models::UserRole;
use heelonvault_rust::repositories::audit_log_repository::SqlxAuditLogRepository;
use heelonvault_rust::repositories::secret_repository::SqlxSecretRepository;
use heelonvault_rust::repositories::team_repository::SqlxTeamRepository;
use heelonvault_rust::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_rust::repositories::vault_repository::SqlxVaultRepository;
use heelonvault_rust::services::admin_service::{AdminService, AdminServiceImpl};
use heelonvault_rust::services::audit_log_service::AuditLogServiceImpl;
use heelonvault_rust::services::audit_service::{AuditAction, AuditService};
use heelonvault_rust::services::auth_policy_service::{AuthPolicyService, SqlxAuthPolicyService};
use heelonvault_rust::services::auth_service::{AuthService, AuthServiceImpl};
use heelonvault_rust::services::backup_application_service::BackupApplicationServiceImpl;
use heelonvault_rust::services::backup_service::{BackupService, BackupServiceImpl};
use heelonvault_rust::services::crypto_service::CryptoServiceImpl;
use heelonvault_rust::services::import_service::ImportServiceImpl;
use heelonvault_rust::services::license_service::LicenseService;
use heelonvault_rust::services::login_history_service::record_successful_login;
use heelonvault_rust::services::password_service::PasswordServiceImpl;
use heelonvault_rust::services::secret_service::SecretServiceImpl;
use heelonvault_rust::services::team_service::TeamServiceImpl;
use heelonvault_rust::services::totp_service::SqliteTotpService;
use heelonvault_rust::services::user_service::{UserService, UserServiceImpl};
use heelonvault_rust::services::vault_service::{VaultKeyEnvelopeRepository, VaultServiceImpl};
use heelonvault_rust::ui::dialogs::login_dialog::{
    AuthenticatedSession, BootstrapServicesContext, LoginDialog,
};
use heelonvault_rust::ui::dialogs::pin_unlock_dialog::{
    PinUnlockDialog, PIN_HARD_TIMEOUT as DIALOG_PIN_HARD_TIMEOUT,
};
use heelonvault_rust::ui::windows::main_window::MainWindow;
use uuid::Uuid;

type VaultServiceHandle = VaultServiceImpl<
    SqlxVaultRepository,
    SqlxVaultEnvelopeRepository,
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
    SqlxVaultEnvelopeRepository,
    SqlxSecretRepository,
    AuthServiceImpl<CryptoServiceImpl>,
    CryptoServiceImpl,
>;
type TotpServiceHandle = SqliteTotpService<AuthServiceImpl<CryptoServiceImpl>, CryptoServiceImpl>;
type AuditLogServiceHandle = AuditLogServiceImpl<SqlxUserRepository, SqlxAuditLogRepository>;
type AdminServiceHandle =
    AdminServiceImpl<SqlxUserRepository, AuthServiceImpl<CryptoServiceImpl>, AuditLogServiceHandle>;
type TeamServiceHandle = TeamServiceImpl<
    SqlxTeamRepository,
    SqlxUserRepository,
    SqlxVaultRepository,
    CryptoServiceImpl,
    AuditLogServiceHandle,
>;
type BackupApplicationServiceHandle =
    BackupApplicationServiceImpl<SqlxUserRepository, BackupServiceImpl>;

struct SqlxVaultEnvelopeRepository {
    pool: SqlitePool,
}

impl SqlxVaultEnvelopeRepository {
    fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl VaultKeyEnvelopeRepository for SqlxVaultEnvelopeRepository {
    async fn get_vault_key_envelope(
        &self,
        vault_id: Uuid,
    ) -> Result<Option<SecretBox<Vec<u8>>>, AppError> {
        let row_opt = sqlx::query("SELECT vault_key_envelope FROM vaults WHERE id = ?1")
            .bind(vault_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row_opt {
            Some(row) => {
                let envelope_bytes: Option<Vec<u8>> = row.try_get("vault_key_envelope")?;
                Ok(envelope_bytes.map(|bytes| SecretBox::new(Box::new(bytes))))
            }
            None => Ok(None),
        }
    }
}

struct AppContext {
    database_path: PathBuf,
    pool: SqlitePool,
    _crypto_service: CryptoServiceImpl,
    auth_service: Arc<AuthServiceImpl<CryptoServiceImpl>>,
    auth_policy_service: Arc<SqlxAuthPolicyService>,
    vault_service: Arc<VaultServiceHandle>,
    secret_service: Arc<SecretServiceHandle>,
    backup_service: Arc<BackupServiceImpl>,
    import_service: Arc<ImportServiceImpl>,
    user_service: Arc<UserServiceHandle>,
    totp_service: Arc<TotpServiceHandle>,
    _audit_log_service: Arc<AuditLogServiceHandle>,
    audit_service: Arc<AuditService>,
    admin_service: Arc<AdminServiceHandle>,
    team_service: Arc<TeamServiceHandle>,
    _backup_app_service: Arc<BackupApplicationServiceHandle>,
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
    admin_service: Arc<AdminServiceHandle>,
    team_service: Arc<TeamServiceHandle>,
}

struct SecondaryServices {
    password_service: PasswordServiceImpl,
    backup_service: Arc<BackupServiceImpl>,
    backup_app_service: Arc<BackupApplicationServiceHandle>,
    import_service: Arc<ImportServiceImpl>,
    totp_service: Arc<TotpServiceHandle>,
    audit_service: Arc<AuditService>,
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
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .with_context(|| format!("failed to open log file {}", log_path.display()))
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

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let startup_flags = StartupFlags::from_args(&args);
    if startup_flags.show_version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Set renderer before any multi-threaded runtime starts.
    // OpenGL avoids verbose Vulkan swapchain warnings on some systems.
    env::set_var("GSK_RENDERER", "gl");

    let _logging_guard = init_logging()?;
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
        info!(
            needs_bootstrap = start_needs_bootstrap,
            "startup check completed successfully"
        );
        println!("startup-check: ok");
        return Ok(());
    }

    run_application(runtime, app_context, start_needs_bootstrap)
}

struct StartupFlags {
    show_version: bool,
    startup_check_only: bool,
}

impl StartupFlags {
    fn from_args(args: &[String]) -> Self {
        Self {
            show_version: args.iter().any(|arg| arg == "--version"),
            startup_check_only: args.iter().any(|arg| arg == "--startup-check"),
        }
    }
}

fn run_application(
    runtime: Arc<tokio::runtime::Runtime>,
    app_context: Arc<AppContext>,
    start_needs_bootstrap: bool,
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
            let bootstrap_ctx_for_dialog = if needs_bootstrap_for_dialog.get() {
                let backup_for_bootstrap = Arc::clone(&context_for_login.backup_service);
                let admin_for_bootstrap = Arc::clone(&context_for_login.admin_service);
                let runtime_for_bootstrap = runtime_for_login.clone();
                Some(BootstrapServicesContext {
                    generate_recovery_key: Arc::new(move || {
                        backup_for_bootstrap
                            .generate_recovery_key()
                            .map(|bundle| bundle.recovery_phrase.expose_secret().to_string())
                    }),
                    do_bootstrap: Arc::new(move |username: String, password_bytes: Vec<u8>| {
                        runtime_for_bootstrap.block_on(async {
                            admin_for_bootstrap
                                .bootstrap_first_admin(
                                    username.as_str(),
                                    SecretBox::new(Box::new(password_bytes)),
                                )
                                .await
                        })
                    }),
                })
            } else {
                None
            };
            let login_license_badge_text = context_for_login
                ._license_service
                .get_cached()
                .map(|license| match license.tier {
                    heelonvault_rust::models::LicenseTier::Community => "Licence free".to_string(),
                    heelonvault_rust::models::LicenseTier::Professional => {
                        format!("Licence pro - {}", license.customer_name)
                    }
                })
                .unwrap_or_else(|| "Licence free".to_string());
            let login_dialog = LoginDialog::new(
                &app_for_login,
                &login_parent_for_dialog,
                runtime_for_login.clone(),
                Arc::clone(&context_for_login.auth_service),
                Arc::clone(&context_for_login.auth_policy_service),
                Arc::clone(&context_for_login.user_service),
                Arc::clone(&context_for_login.totp_service),
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

                    context_for_restore
                        .backup_service
                        .import_hvb_with_recovery_key(
                            backup_file_path.as_path(),
                            &secrecy::SecretString::new(recovery_phrase.into_boxed_str()),
                            staging_path.as_path(),
                        )?;

                    runtime_for_restore_task
                        .block_on(async {
                            apply_restored_login_password(
                                staging_path.as_path(),
                                new_password.as_str(),
                            )
                            .await
                        })
                        .map_err(|error| {
                            AppError::Storage(format!(
                                "failed to update restored password envelope: {error}"
                            ))
                        })?;

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
                    Ok(())
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
                        .map(|u| matches!(u.role, heelonvault_rust::models::UserRole::Admin))
                        .unwrap_or(false);
                    if let Some(language) =
                        user_profile.as_ref().map(|u| u.preferred_language.clone())
                    {
                        let _ = heelonvault_rust::i18n::set_language(language.as_str());
                    }

                    let main_window_build_started = Instant::now();
                    let license_badge_text = context_for_success
                        ._license_service
                        .get_cached()
                        .map(|license| match license.tier {
                            heelonvault_rust::models::LicenseTier::Community => {
                                "Licence free".to_string()
                            }
                            heelonvault_rust::models::LicenseTier::Professional => {
                                format!("Licence pro - {}", license.customer_name)
                            }
                        })
                        .unwrap_or_else(|| "Licence free".to_string());
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
                        Arc::clone(&context_for_success.audit_service),
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

                    let refresh_entries_started = Instant::now();
                    main_for_success.refresh_entries();
                    info!(
                        elapsed_ms = refresh_entries_started.elapsed().as_millis() as u64,
                        total_elapsed_ms = login_success_started.elapsed().as_millis() as u64,
                        "login flow trace: refresh_entries invoked"
                    );

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
        "heelonvault_rust::services::crypto_service",
        "heelonvault_rust::services::secret_service",
        "heelonvault_rust::services::auth_service",
        "heelonvault_rust::services::vault_service",
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
    fs::create_dir_all(&log_dir_path)
        .with_context(|| format!("failed to create log directory {}", log_dir_path.display()))?;

    let rolling_writer = DailyLogFileWriter::new(log_dir_path.clone(), "heelonvault")?;
    let (file_writer, guard) = tracing_appender::non_blocking(rolling_writer);

    let is_debug_logging = base_filter_spec.to_ascii_lowercase().contains("debug");
    let is_dev_mode = cfg!(debug_assertions);

    if is_dev_mode {
        let console_layer = tracing_subscriber::fmt::layer()
            .pretty()
            .with_writer(std::io::stdout)
            .with_target(false)
            .with_timer(LocalRfc3339Timer);
        let file_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_ansi(false)
            .with_writer(file_writer)
            .with_target(is_debug_logging)
            .with_line_number(is_debug_logging)
            .with_file(is_debug_logging)
            .with_timer(LocalRfc3339Timer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .try_init()
            .map_err(|error| anyhow!("failed to initialize tracing subscriber: {error}"))?;
    } else {
        let console_layer = tracing_subscriber::fmt::layer()
            .compact()
            .with_writer(std::io::stdout)
            .with_target(false)
            .with_timer(LocalRfc3339Timer);
        let file_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_ansi(false)
            .with_writer(file_writer)
            .with_target(is_debug_logging)
            .with_line_number(is_debug_logging)
            .with_file(is_debug_logging)
            .with_timer(LocalRfc3339Timer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .try_init()
            .map_err(|error| anyhow!("failed to initialize tracing subscriber: {error}"))?;
    }

    Ok(guard)
}

fn build_primary_services(pool: &SqlitePool) -> PrimaryServices {
    let crypto_service = CryptoServiceImpl::default();
    let auth_service = Arc::new(AuthServiceImpl::new(CryptoServiceImpl::default()));
    let audit_log_service = Arc::new(AuditLogServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        SqlxAuditLogRepository::new(pool.clone()),
    ));
    let auth_policy_service = Arc::new(SqlxAuthPolicyService::new(pool.clone()));
    let vault_service = Arc::new(VaultServiceImpl::new(
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultEnvelopeRepository::new(pool.clone()),
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
        SqlxVaultEnvelopeRepository::new(pool.clone()),
        SqlxSecretRepository::new(pool.clone()),
        Arc::clone(&auth_service),
        CryptoServiceImpl::default(),
    ));
    let admin_service = Arc::new(AdminServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        Arc::clone(&auth_service),
        Arc::clone(&audit_log_service),
    ));
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
    ));
    let import_service = Arc::new(ImportServiceImpl::new());
    let totp_service = Arc::new(SqliteTotpService::new(
        pool.clone(),
        Arc::clone(&auth_service),
        CryptoServiceImpl::default(),
        "HeelonVault",
    ));
    let audit_service = Arc::new(AuditService::new(pool.clone()));
    let mut license_service = LicenseService::new();

    match license_service.load_license().await {
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

    SecondaryServices {
        password_service,
        backup_service,
        backup_app_service,
        import_service,
        totp_service,
        audit_service,
        license_service: Arc::new(license_service),
    }
}

async fn initialize_app_context() -> Result<AppStartMode> {
    let database_path = resolve_database_path()?;
    if let Some(parent) = database_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create database directory {}", parent.display())
            })?;
        }
    }

    let connect_options = SqliteConnectOptions::new()
        .filename(&database_path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(connect_options)
        .await
        .with_context(|| {
            format!(
                "failed to open sqlite database at {}",
                database_path.display()
            )
        })?;

    sqlx::migrate::Migrator::new(Path::new("./migrations"))
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
        _crypto_service: primary.crypto_service,
        auth_service: primary.auth_service,
        auth_policy_service: primary.auth_policy_service,
        vault_service: primary.vault_service,
        secret_service: primary.secret_service,
        backup_service: secondary.backup_service,
        import_service: secondary.import_service,
        user_service: primary.user_service,
        totp_service: secondary.totp_service,
        _audit_log_service: primary.audit_log_service,
        audit_service: secondary.audit_service,
        admin_service: primary.admin_service,
        team_service: primary.team_service,
        _backup_app_service: secondary.backup_app_service,
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
    if let Some(parent) = database_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create database directory {}", parent.display())
            })?;
        }
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

async fn apply_restored_login_password(database_path: &Path, new_password: &str) -> Result<()> {
    let connect_options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(false);
    let pool = SqlitePool::connect_with(connect_options)
        .await
        .with_context(|| {
            format!(
                "failed to open restored database at {}",
                database_path.display()
            )
        })?;

    let selected_user = sqlx::query(
        "SELECT username FROM users ORDER BY CASE WHEN role = ?1 THEN 0 ELSE 1 END, rowid LIMIT 1",
    )
    .bind(UserRole::Admin.to_db_str())
    .fetch_optional(&pool)
    .await
    .context("failed to resolve restored user for password reset")?
    .ok_or_else(|| anyhow!("restored database does not contain any user"))?;

    let username: String = selected_user
        .try_get("username")
        .context("failed to read restored username")?;

    let auth_service = AuthServiceImpl::new(CryptoServiceImpl::default());
    auth_service
        .create_user(
            username.as_str(),
            SecretBox::new(Box::new(new_password.as_bytes().to_vec())),
        )
        .await
        .map_err(|error| anyhow!("failed to stage restored password envelope: {error}"))?;

    let password_envelope = auth_service
        .get_password_envelope(username.as_str())
        .await
        .map_err(|error| anyhow!("failed to export restored password envelope: {error}"))?;

    sqlx::query("UPDATE users SET password_envelope = ?1 WHERE username = ?2")
        .bind(password_envelope.expose_secret().as_slice())
        .bind(username.as_str())
        .execute(&pool)
        .await
        .context("failed to persist restored password envelope")?;

    let _ = sqlx::query("DELETE FROM auth_policy").execute(&pool).await;
    pool.close().await;
    Ok(())
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
    if current_dir.file_name().is_some_and(|name| name == "rust") {
        if let Some(project_root) = current_dir.parent() {
            return project_root.join("data").join(db_name);
        }
    }

    current_dir.join("data").join(db_name)
}

fn resolve_default_log_dir(current_dir: &Path) -> PathBuf {
    resolve_default_log_dir_for(current_dir, resolve_platform_runtime_root())
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
    if cfg!(target_os = "windows") {
        dirs::data_local_dir().map(|path| path.join("heelonvault"))
    } else if cfg!(target_os = "macos") {
        dirs::data_dir().map(|path| path.join("heelonvault"))
    } else {
        None
    }
}

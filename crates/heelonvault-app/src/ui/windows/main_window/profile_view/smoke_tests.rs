//! Native GTK check of the real profile page: Community/Professional separation for each kind
//! of account, plus optional snapshots (`HEELONVAULT_UI_SNAPSHOT_DIR`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use heelonvault_core::i18n::tr;
use heelonvault_core::models::UserRole;
use heelonvault_core::repositories::secret_repository::SqlxSecretRepository;
use heelonvault_core::repositories::team_repository::SqlxTeamRepository;
use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::repositories::vault_repository::SqlxVaultRepository;
use heelonvault_core::services::audit_log_service::NoOpAuditLogService;
use heelonvault_core::services::auth_policy_service::SqlxAuthPolicyService;
use heelonvault_core::services::auth_service::AuthServiceImpl;
use heelonvault_core::services::backup_application_service::BackupApplicationServiceImpl;
use heelonvault_core::services::backup_service::BackupServiceImpl;
use heelonvault_core::services::crypto_service::CryptoServiceImpl;
use heelonvault_core::services::import_service::ImportServiceImpl;
use heelonvault_core::services::secret_service::SecretServiceImpl;
use heelonvault_core::services::totp_service::SqliteTotpService;
use heelonvault_core::services::user_service::UserServiceImpl;
use heelonvault_core::services::vault_service::VaultServiceImpl;
use libadwaita as adw;
use libadwaita::prelude::AdwApplicationWindowExt;
use uuid::Uuid;

use super::build_profile_view;
use crate::ui::entitlements::{Edition, Entitlements};

/// Every label text under `widget`, including row titles and subtitles.
fn texts(widget: &gtk4::Widget, found: &mut Vec<String>) {
    if let Some(label) = widget.downcast_ref::<gtk4::Label>() {
        found.push(label.text().to_string());
    }
    if let Some(button) = widget.downcast_ref::<gtk4::Button>()
        && let Some(label) = button.label()
    {
        found.push(label.to_string());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        texts(&current, found);
        child = current.next_sibling();
    }
}

fn drain(context: &glib::MainContext, millis: u64) {
    let until = std::time::Instant::now() + Duration::from_millis(millis);
    while std::time::Instant::now() < until {
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Run by `native_gtk_smoke` (main_window/mod.rs), after GTK and the theme are initialised.
pub(in crate::ui::windows::main_window) fn native_profile_scenario()
-> Result<(), Box<dyn std::error::Error>> {
    let language = crate::ui::doc_screenshots::language().unwrap_or_else(|| "fr".to_string());
    assert!(heelonvault_core::i18n::set_language(&language));

    let runtime = tokio::runtime::Runtime::new()?;
    let pool = runtime.block_on(async {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate::Migrator::new(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations"
        )))
        .await?
        .run(&pool)
        .await?;
        Ok::<_, Box<dyn std::error::Error>>(pool)
    })?;
    let user_id = Uuid::new_v4();
    let user_repo = Arc::new(SqlxUserRepository::new(pool.clone()));
    runtime.block_on(async {
        user_repo
            .create_user_db(user_id, "camille", &UserRole::Admin)
            .await?;
        user_repo
            .update_user_profile(
                user_id,
                Some("camille.martin@example.com"),
                Some("Camille Martin"),
                Some(&language),
                Some(false),
            )
            .await
    })?;

    let crypto = Arc::new(CryptoServiceImpl::default());
    let auth = Arc::new(AuthServiceImpl::new(CryptoServiceImpl::default()));
    let audit = Arc::new(NoOpAuditLogService);
    let user_service = Arc::new(UserServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxSecretRepository::new(pool.clone()),
        Arc::clone(&auth),
        CryptoServiceImpl::default(),
    ));
    let totp = Arc::new(SqliteTotpService::new(
        pool.clone(),
        Arc::clone(&auth),
        CryptoServiceImpl::default(),
        "HeelonVault",
    ));
    let policy = Arc::new(SqlxAuthPolicyService::new(pool.clone()));
    let backup = Arc::new(BackupServiceImpl::new());
    let backup_app = Arc::new(BackupApplicationServiceImpl::new(
        SqlxUserRepository::new(pool.clone()),
        BackupServiceImpl::new(),
        pool.clone(),
    ));
    let import = Arc::new(ImportServiceImpl::new());
    let secrets = Arc::new(SecretServiceImpl::new(
        SqlxSecretRepository::new(pool.clone()),
        CryptoServiceImpl::default(),
        Arc::clone(&audit),
    ));
    let vaults = Arc::new(VaultServiceImpl::new(
        SqlxVaultRepository::new(pool.clone()),
        SqlxVaultRepository::new(pool.clone()),
        SqlxUserRepository::new(pool.clone()),
        SqlxTeamRepository::new(pool.clone()),
        Arc::clone(&audit),
        CryptoServiceImpl::default(),
    ));

    let context = glib::MainContext::default();
    let cases = [
        (
            "community-admin",
            Entitlements::new(Edition::Community, true),
        ),
        (
            "professional-admin",
            Entitlements::new(Edition::Professional, true),
        ),
        (
            "community-user",
            Entitlements::new(Edition::Community, false),
        ),
    ];
    for (name, entitlements) in cases {
        let window = adw::ApplicationWindow::builder()
            .default_width(1100)
            .default_height(if crate::ui::doc_screenshots::language().is_some() {
                900
            } else {
                1500
            })
            .build();
        let opened = Rc::new(Cell::new(0));
        let opened_users = Rc::clone(&opened);
        let page = build_profile_view(
            window.clone(),
            runtime.handle().clone(),
            Arc::clone(&user_service),
            Arc::clone(&user_repo),
            Arc::clone(&crypto),
            Arc::clone(&totp),
            Arc::clone(&policy),
            Arc::clone(&backup),
            Arc::clone(&backup_app),
            Arc::clone(&import),
            Arc::clone(&secrets),
            Arc::clone(&vaults),
            std::path::PathBuf::from("unused.db"),
            user_id,
            entitlements,
            Rc::new(move || opened_users.set(opened_users.get() + 1)),
            Rc::new(|| {}),
            gtk4::MenuButton::new(),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(300)),
            Rc::new(RefCell::new(None)),
            Rc::new(Cell::new(false)),
            Rc::new(RefCell::new(None)),
            Rc::new(RefCell::new(vec![1; 32])),
            Rc::new(RefCell::new(None)),
            Rc::new(Cell::new(false)),
            Rc::new(|| {}),
            Rc::new(|| {}),
            Rc::new(|_| {}),
        );
        window.set_content(Some(&page.container));
        window.present();
        drain(&context, 600);

        let mut found = Vec::new();
        texts(page.container.upcast_ref(), &mut found);
        let has = |key: &str| found.contains(&tr(key));
        assert!(
            has("profile-section-protection"),
            "{name}: protection group"
        );
        assert!(
            found.contains(&"Camille Martin".to_string()),
            "{name}: profile loaded"
        );
        match name {
            "professional-admin" => {
                assert!(has("profile-admin-users-subtitle"), "{name}");
                assert!(!has("profile-admin-offer-title"), "{name}");
            }
            "community-admin" => {
                // Community: an informative row only, with no entry to the Pro pages.
                assert!(has("profile-admin-offer-title"), "{name}");
                assert!(!has("profile-admin-users-subtitle"), "{name}");
                assert!(!has("profile-admin-teams-subtitle"), "{name}");
            }
            _ => {
                assert!(!has("profile-admin-offer-title"), "{name}");
                assert!(!has("profile-admin-users-subtitle"), "{name}");
            }
        }
        assert_eq!(opened.get(), 0);
        let capture_name = match name {
            "community-admin" => "profile-community",
            "professional-admin" => "profile-professional",
            _ => "profile-user",
        };
        crate::ui::doc_screenshots::capture(&window, capture_name)?;
        if crate::ui::doc_screenshots::language().is_some() {
            let scroll = page.container.vadjustment();
            scroll.set_value(scroll.upper() - scroll.page_size());
            drain(&context, 400);
            crate::ui::doc_screenshots::capture(&window, &format!("{capture_name}-data"))?;
            scroll.set_value(0.0);
        }

        if let Some(dir) = std::env::var_os("HEELONVAULT_UI_SNAPSHOT_DIR") {
            let paintable = gtk4::WidgetPaintable::new(Some(&window));
            let snapshot = gtk4::Snapshot::new();
            paintable.snapshot(
                &snapshot,
                f64::from(window.width()),
                f64::from(window.height()),
            );
            let node = snapshot.to_node().ok_or("empty GTK snapshot")?;
            let surface = window.surface().ok_or("missing surface")?;
            let renderer = gtk4::gsk::Renderer::for_surface(&surface).ok_or("missing renderer")?;
            renderer
                .render_texture(&node, None)
                .save_to_png(std::path::PathBuf::from(dir).join(format!("profile-{name}.png")))?;
            renderer.unrealize();
        }
        window.close();
        drain(&context, 50);
    }
    Ok(())
}

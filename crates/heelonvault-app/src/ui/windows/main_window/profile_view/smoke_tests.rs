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
use heelonvault_core::services::pin_cache_service::PinCache;
use heelonvault_core::services::secret_service::SecretServiceImpl;
use heelonvault_core::services::totp_service::SqliteTotpService;
use heelonvault_core::services::user_service::UserServiceImpl;
use heelonvault_core::services::vault_service::VaultServiceImpl;
use libadwaita as adw;
use libadwaita::prelude::*;
use uuid::Uuid;

use super::build_profile_view;
use crate::ui::entitlements::{Edition, Entitlements};
use crate::ui::windows::main_window::window::{events, pin_badge, views};

fn widgets(root: &gtk4::Widget) -> Vec<gtk4::Widget> {
    let mut found = vec![root.clone()];
    let mut child = root.first_child();
    while let Some(current) = child {
        found.extend(widgets(&current));
        child = current.next_sibling();
    }
    found
}

fn button(root: &gtk4::Widget, key: &str) -> Result<gtk4::Button, Box<dyn std::error::Error>> {
    widgets(root)
        .iter()
        .filter_map(|widget| widget.downcast_ref::<gtk4::Button>())
        .find(|button| button.label().as_deref() == Some(tr(key).as_str()))
        .cloned()
        .ok_or_else(|| format!("missing button: {key}").into())
}

fn open_pin_dialog(
    trigger: &gtk4::Button,
    parent: &adw::ApplicationWindow,
) -> Result<gtk4::Window, Box<dyn std::error::Error>> {
    trigger.emit_clicked();
    gtk4::Window::list_toplevels()
        .iter()
        .filter_map(|widget| widget.downcast_ref::<gtk4::Window>())
        .find(|window| window.transient_for().as_ref() == Some(parent.upcast_ref()))
        .cloned()
        .ok_or_else(|| "missing PIN dialog".into())
}

fn save_pin(dialog: &gtk4::Window) -> Result<(), Box<dyn std::error::Error>> {
    let entries: Vec<_> = widgets(dialog.upcast_ref())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk4::Entry>().ok())
        .filter(|entry| entry.input_purpose() == gtk4::InputPurpose::Pin)
        .collect();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        entry.set_text("123456");
    }
    button(dialog.upcast_ref(), "pin-setup-save")?.emit_clicked();
    Ok(())
}

fn assert_pin_state(root: &gtk4::Widget, header: &gtk4::Label, active: bool) {
    let expected = tr(if active {
        "pin-status-active"
    } else {
        "pin-status-inactive"
    });
    assert_eq!(header.text().as_str(), expected);
    let all = widgets(root);
    let rows: Vec<_> = all
        .iter()
        .filter_map(|widget| widget.downcast_ref::<adw::ActionRow>())
        .filter(|row| row.title().as_str() == tr("profile-pin-title"))
        .collect();
    assert_eq!(rows.len(), 1);
    let row_widgets = widgets(rows[0].upcast_ref());
    let badges: Vec<_> = row_widgets
        .iter()
        .filter_map(|widget| widget.downcast_ref::<gtk4::Label>())
        .filter(|label| label.has_css_class("status-role-pill"))
        .collect();
    assert_eq!(badges.len(), 1);
    for badge in badges {
        assert_eq!(badge.text().as_str(), expected);
        assert_eq!(badge.has_css_class("status-role-user"), active);
        assert_eq!(badge.has_css_class("status-role-disabled"), !active);
    }
    let icons: Vec<_> = all
        .iter()
        .filter_map(|widget| widget.downcast_ref::<gtk4::Image>())
        .filter(|image| {
            image
                .next_sibling()
                .and_downcast::<gtk4::Label>()
                .is_some_and(|label| label.text().as_str() == tr("profile-protection-pin"))
        })
        .collect();
    assert_eq!(icons.len(), 1);
    assert_eq!(
        icons[0].icon_name().as_deref(),
        Some(if active {
            "hv-check-symbolic"
        } else {
            "hv-warning-symbolic"
        })
    );
}

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
        let pin_cache = Rc::new(RefCell::new(None));
        let session_key = Rc::new(RefCell::new(vec![1; 32]));
        let on_pin_state: Rc<RefCell<Option<Rc<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));
        let state_for_profile = Rc::downgrade(&on_pin_state);
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
            Rc::clone(&session_key),
            Rc::clone(&pin_cache),
            Rc::new(Cell::new(false)),
            Rc::new(|| {}),
            Rc::new(|| {}),
            Rc::new(move |active| {
                if let Some(state) = state_for_profile.upgrade()
                    && let Some(refresh) = state.borrow().as_ref()
                {
                    refresh(active);
                }
            }),
        );
        let (header_button, header_label) = views::build_pin_status_badge();
        let refresh_pin = pin_badge::build_pin_state_callback(
            header_label.clone(),
            header_button.clone(),
            Rc::clone(&pin_cache),
            Rc::clone(&page.refresh_pin_state),
        );
        *on_pin_state.borrow_mut() = Some(Rc::clone(&refresh_pin));
        events::setup_pin_badge_handler(
            &header_button,
            window.clone(),
            session_key,
            Rc::clone(&pin_cache),
            Rc::clone(&on_pin_state),
            user_id,
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
        let root = page.container.upcast_ref();
        let profile_button = button(root, "profile-pin-configure")?;
        assert_pin_state(root, &header_label, false);
        for trigger in [&header_button, &profile_button] {
            // Cancelling setup must not activate a PIN.
            open_pin_dialog(trigger, &window)?.close();
            assert_pin_state(root, &header_label, false);
            save_pin(&open_pin_dialog(trigger, &window)?)?;
            assert!(pin_cache.borrow().is_some());
            assert_pin_state(root, &header_label, true);
            // Replacing an active PIN keeps both surfaces in sync.
            save_pin(&open_pin_dialog(trigger, &window)?)?;
            assert_pin_state(root, &header_label, true);
            let other = if trigger == &header_button {
                &profile_button
            } else {
                &header_button
            };
            let dialog = open_pin_dialog(other, &window)?;
            button(dialog.upcast_ref(), "pin-setup-disable")?.emit_clicked();
            assert!(pin_cache.borrow().is_none());
            assert_pin_state(root, &header_label, false);
        }
        save_pin(&open_pin_dialog(&header_button, &window)?)?;
        let profile_badge = widgets(root)
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk4::Label>().ok())
            .find(|label| {
                label.has_css_class("status-role-pill")
                    && label.text().as_str() == tr("pin-status-active")
            })
            .ok_or("missing profile PIN badge")?;
        let other_language = if language == "fr" { "en" } else { "fr" };
        assert!(heelonvault_core::i18n::set_language(other_language));
        refresh_pin(true);
        assert_eq!(header_label.text().as_str(), tr("pin-status-active"));
        assert_eq!(profile_badge.text().as_str(), tr("pin-status-active"));
        assert!(heelonvault_core::i18n::set_language(&language));
        refresh_pin(true);
        // Exercise the same expiry renderer as the countdown without waiting twelve hours.
        pin_badge::expire_for_test(
            &header_label,
            &header_button,
            &pin_cache,
            page.refresh_pin_state.as_ref(),
        );
        assert!(pin_cache.borrow().is_none());
        assert_pin_state(root, &header_label, false);
        refresh_pin(false);
        // An activation notification without a usable cache must not show "active".
        refresh_pin(true);
        assert_pin_state(root, &header_label, false);
        *pin_cache.borrow_mut() = Some(PinCache::wrap(
            &secrecy::SecretBox::new(Box::new(vec![1; 32])),
            "123456",
            user_id,
        )?);
        refresh_pin(true);
        for _ in 0..3 {
            let result = pin_cache
                .borrow_mut()
                .as_mut()
                .ok_or("missing PIN cache")?
                .try_unwrap("000000");
            assert!(result.is_err());
        }
        refresh_pin(true);
        assert!(pin_cache.borrow().is_none());
        assert_pin_state(root, &header_label, false);
        refresh_pin(false);
        window.close();
        drain(&context, 50);
    }
    Ok(())
}

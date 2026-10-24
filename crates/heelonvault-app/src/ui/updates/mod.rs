//! Optional release discovery. Never receives session, vault or license data.
mod check;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use gtk4::{glib, prelude::*};
use heelonvault_core::i18n::{I18nArg, tr, tr_args};
use libadwaita as adw;
use libadwaita::prelude::*;
use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;

const DAY: i64 = 24 * 60 * 60;

#[derive(Default, Serialize, Deserialize)]
struct Preferences {
    automatic: Option<bool>,
    last_attempt: Option<i64>,
    latest: Option<String>,
}

fn read_preferences(path: &Path) -> anyhow::Result<Preferences> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(error) => Err(error.into()),
    }
}

fn save_preferences(path: &Path, preferences: &Preferences) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(preferences)?)?;
    Ok(())
}

fn due(preferences: &Preferences, now: i64) -> bool {
    preferences.automatic == Some(true)
        && preferences
            .last_attempt
            .is_none_or(|last| now.saturating_sub(last) >= DAY)
}

#[derive(Clone, Default)]
enum Status {
    #[default]
    Idle,
    Checking,
    Current,
    Available(check::StableRelease),
    Unavailable,
    StorageError,
}

struct Checker {
    preferences: RefCell<Preferences>,
    path: PathBuf,
    runtime: RefCell<Option<Handle>>,
    status: RefCell<Status>,
    observers: RefCell<Vec<Weak<dyn Fn()>>>,
    generation: Cell<u64>,
    task: RefCell<Option<tokio::task::AbortHandle>>,
    dismissed: Cell<bool>,
}

thread_local! {
    static SHARED: Rc<Checker> = Checker::load();
}

impl Checker {
    fn load() -> Rc<Self> {
        let path = super::window_sizing::ui_state_file_path("update_preferences.json");
        let (preferences, status) = match read_preferences(&path) {
            Ok(preferences) => (preferences, Status::Idle),
            Err(error) => {
                tracing::warn!(%error, "Unable to load update preferences; network checks disabled");
                (Preferences::default(), Status::StorageError)
            }
        };
        Rc::new(Self {
            preferences: RefCell::new(preferences),
            path,
            runtime: RefCell::new(None),
            status: RefCell::new(status),
            observers: RefCell::new(Vec::new()),
            generation: Cell::new(0),
            task: RefCell::new(None),
            dismissed: Cell::new(false),
        })
    }

    fn notify(&self) {
        let callbacks: Vec<_> = {
            let mut observers = self.observers.borrow_mut();
            observers.retain(|observer| observer.strong_count() > 0);
            observers.iter().filter_map(Weak::upgrade).collect()
        };
        for callback in callbacks {
            callback();
        }
    }

    fn save(&self) -> bool {
        if let Err(error) = save_preferences(&self.path, &self.preferences.borrow()) {
            tracing::warn!(%error, "Unable to save update preferences");
            *self.status.borrow_mut() = Status::StorageError;
            self.dismissed.set(false);
            self.notify();
            return false;
        }
        true
    }

    fn consent(self: &Rc<Self>, enabled: bool) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        let previous = self.preferences.borrow().automatic;
        self.preferences.borrow_mut().automatic = Some(enabled);
        if matches!(
            *self.status.borrow(),
            Status::Checking | Status::StorageError
        ) {
            *self.status.borrow_mut() = Status::Idle;
        }
        self.dismissed.set(!enabled);
        if !self.save() {
            self.preferences.borrow_mut().automatic = previous;
            self.notify();
            return;
        }
        self.notify();
        if enabled {
            self.check(false);
        }
    }

    fn check(self: &Rc<Self>, manual: bool) {
        self.check_with(manual, check::latest());
    }

    fn check_with(
        self: &Rc<Self>,
        manual: bool,
        request: impl std::future::Future<Output = anyhow::Result<check::StableRelease>>
        + Send
        + 'static,
    ) {
        if matches!(*self.status.borrow(), Status::Checking) {
            return;
        }
        let now = chrono::Utc::now().timestamp();
        if !manual && !due(&self.preferences.borrow(), now) {
            return;
        }
        let Some(runtime) = self.runtime.borrow().clone() else {
            *self.status.borrow_mut() = Status::Unavailable;
            self.notify();
            return;
        };
        self.preferences.borrow_mut().last_attempt = Some(now);
        if !self.save() {
            return;
        }
        *self.status.borrow_mut() = Status::Checking;
        self.dismissed.set(false);
        self.notify();
        let generation = self.generation.get();
        let task = runtime.spawn(request);
        *self.task.borrow_mut() = Some(task.abort_handle());
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = task.await;
            let Some(checker) = weak.upgrade() else {
                return;
            };
            if generation != checker.generation.get() {
                return;
            }
            checker.task.borrow_mut().take();
            match result {
                Ok(Ok(latest)) => {
                    checker.preferences.borrow_mut().latest = Some(latest.tag().to_string());
                    match Version::parse(env!("CARGO_PKG_VERSION")) {
                        Ok(installed) => {
                            *checker.status.borrow_mut() =
                                if check::newer(&latest.version, &installed) {
                                    Status::Available(latest)
                                } else {
                                    Status::Current
                                };
                        }
                        Err(error) => {
                            tracing::error!(%error, "Invalid installed product version");
                            *checker.status.borrow_mut() = Status::Unavailable;
                        }
                    }
                    if !checker.save() {
                        return;
                    }
                }
                Ok(Err(error)) => {
                    tracing::warn!(%error, "Update check unavailable");
                    *checker.status.borrow_mut() = Status::Unavailable;
                }
                Err(error) => {
                    tracing::warn!(%error, "Update check worker failed");
                    *checker.status.borrow_mut() = Status::Unavailable;
                }
            }
            checker.notify();
        });
    }
}

/// Called once at application activation, before login. No request without saved consent.
pub fn initialize(runtime: Handle) {
    SHARED.with(|checker| {
        if checker.runtime.borrow().is_some() {
            return;
        }
        *checker.runtime.borrow_mut() = Some(runtime);
        // Only a cached newer release is useful; a cache is not proof of being up to date.
        if checker.preferences.borrow().automatic == Some(true)
            && let Some(latest) = checker.preferences.borrow().latest.as_deref()
        {
            match (
                check::StableRelease::from_tag(latest),
                Version::parse(env!("CARGO_PKG_VERSION")),
            ) {
                (Ok(latest), Ok(installed)) if check::newer(&latest.version, &installed) => {
                    *checker.status.borrow_mut() = Status::Available(latest);
                }
                (Err(error), _) => tracing::warn!(%error, "Invalid cached update version"),
                _ => {}
            }
        }
        checker.check(false);
    });
}

#[derive(Clone)]
pub struct UpdateSection {
    pub group: adw::PreferencesGroup,
    pub refresh_i18n: Rc<dyn Fn()>,
}

fn status_text(status: &Status) -> String {
    match status {
        Status::Idle => tr("updates-idle"),
        Status::Checking => tr("updates-checking"),
        Status::Current => tr("updates-current"),
        Status::Available(release) => tr_args(
            "updates-available",
            &[("version", I18nArg::Str(&release.version.to_string()))],
        ),
        Status::Unavailable => tr("updates-unavailable"),
        Status::StorageError => tr("updates-storage-error"),
    }
}

pub fn build_preferences() -> UpdateSection {
    let checker = SHARED.with(Rc::clone);
    build_preferences_for(checker)
}

fn build_preferences_for(checker: Rc<Checker>) -> UpdateSection {
    let group = adw::PreferencesGroup::new();
    group.add_css_class("profile-group");
    let automatic = adw::ActionRow::new();
    automatic.set_subtitle_lines(0);
    let toggle = gtk4::Switch::builder().valign(gtk4::Align::Center).build();
    automatic.add_suffix(&toggle);
    automatic.set_activatable_widget(Some(&toggle));
    automatic.add_prefix(&gtk4::Image::from_icon_name("hv-globe-symbolic"));
    group.add(&automatic);
    let status = adw::ActionRow::new();
    status.set_subtitle_lines(0);
    status.add_prefix(&gtk4::Image::from_icon_name("hv-sparkle-symbolic"));
    let actions = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    actions.set_valign(gtk4::Align::Center);
    let spinner = gtk4::Spinner::new();
    let check = gtk4::Button::new();
    check.add_css_class("suggested-action");
    let releases = gtk4::Button::new();
    releases.add_css_class("flat");
    actions.append(&spinner);
    actions.append(&check);
    actions.append(&releases);
    status.add_suffix(&actions);
    group.add(&status);
    let updating = Rc::new(Cell::new(false));
    let render: Rc<dyn Fn()> = {
        let checker = Rc::clone(&checker);
        let group = group.clone();
        let automatic = automatic.clone();
        let toggle = toggle.clone();
        let status = status.clone();
        let check = check.clone();
        let releases = releases.clone();
        let updating = Rc::clone(&updating);
        Rc::new(move || {
            group.set_title(&tr("updates-title"));
            group.set_description(Some(&tr("updates-description")));
            automatic.set_title(&tr("updates-automatic"));
            automatic.set_subtitle(&tr("updates-privacy"));
            updating.set(true);
            toggle.set_active(checker.preferences.borrow().automatic == Some(true));
            updating.set(false);
            let current = checker.status.borrow().clone();
            status.set_title(&status_text(&current));
            status.set_subtitle(&tr_args(
                "updates-installed",
                &[("version", I18nArg::Str(env!("CARGO_PKG_VERSION")))],
            ));
            let busy = matches!(current, Status::Checking);
            spinner.set_spinning(busy);
            spinner.set_visible(busy);
            check.set_sensitive(!busy);
            check.set_label(&tr("updates-check"));
            check.set_tooltip_text(Some(&tr("updates-check-hint")));
            releases.set_label(&tr("updates-releases"));
        })
    };
    checker.observers.borrow_mut().push(Rc::downgrade(&render));
    let for_toggle = Rc::clone(&checker);
    toggle.connect_active_notify(move |toggle| {
        if !updating.get() {
            for_toggle.consent(toggle.is_active());
        }
    });
    let for_check = Rc::clone(&checker);
    check.connect_clicked(move |_| for_check.check(true));
    releases.connect_clicked(move |button| {
        let parent = button.root().and_downcast::<gtk4::Window>();
        gtk4::show_uri(parent.as_ref(), check::RELEASES, gtk4::gdk::CURRENT_TIME);
    });
    render();
    UpdateSection {
        group,
        refresh_i18n: render,
    }
}

pub struct UpdateBanner {
    pub widget: gtk4::Box,
    pub refresh_i18n: Rc<dyn Fn()>,
}

pub fn build_banner() -> UpdateBanner {
    let checker = SHARED.with(Rc::clone);
    build_banner_for(checker)
}

fn build_banner_for(checker: Rc<Checker>) -> UpdateBanner {
    let widget = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
    widget.add_css_class("update-banner");
    let icon = gtk4::Image::from_icon_name("hv-sparkle-symbolic");
    icon.set_pixel_size(28);
    icon.add_css_class("update-banner-icon");
    widget.append(&icon);
    let copy = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    copy.set_hexpand(true);
    let title = gtk4::Label::new(None);
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    title.add_css_class("heading");
    let body = gtk4::Label::new(None);
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    body.add_css_class("dim-label");
    copy.append(&title);
    copy.append(&body);
    widget.append(&copy);
    let actions = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    actions.set_valign(gtk4::Align::Center);
    let primary = gtk4::Button::new();
    primary.add_css_class("suggested-action");
    let secondary = gtk4::Button::new();
    secondary.add_css_class("flat");
    actions.append(&primary);
    actions.append(&secondary);
    widget.append(&actions);
    let render: Rc<dyn Fn()> = {
        let checker = Rc::clone(&checker);
        let widget = widget.clone();
        let primary = primary.clone();
        let secondary = secondary.clone();
        Rc::new(move || {
            let consent = checker.preferences.borrow().automatic.is_none();
            let available = matches!(*checker.status.borrow(), Status::Available(_));
            let storage_error = matches!(*checker.status.borrow(), Status::StorageError);
            widget.set_visible((consent || available || storage_error) && !checker.dismissed.get());
            title.set_text(&if storage_error {
                tr("updates-storage-error")
            } else if consent {
                tr("updates-consent-title")
            } else {
                status_text(&checker.status.borrow())
            });
            body.set_text(&tr(if consent || storage_error {
                "updates-privacy"
            } else {
                "updates-banner-body"
            }));
            primary.set_label(&tr(if consent {
                "updates-enable"
            } else if storage_error {
                "updates-check"
            } else {
                "updates-download"
            }));
            secondary.set_label(&tr(if consent {
                "updates-not-now"
            } else {
                "updates-later"
            }));
        })
    };
    checker.observers.borrow_mut().push(Rc::downgrade(&render));
    let for_primary = Rc::clone(&checker);
    primary.connect_clicked(move |button| {
        let needs_consent = for_primary.preferences.borrow().automatic.is_none();
        let status = for_primary.status.borrow().clone();
        if needs_consent {
            for_primary.consent(true);
        } else if let Status::Available(release) = status {
            let parent = button.root().and_downcast::<gtk4::Window>();
            gtk4::show_uri(parent.as_ref(), &release.url(), gtk4::gdk::CURRENT_TIME);
        } else {
            for_primary.check(true);
        }
    });
    let for_secondary = Rc::clone(&checker);
    secondary.connect_clicked(move |_| {
        let needs_consent = for_secondary.preferences.borrow().automatic.is_none();
        let storage_error = matches!(*for_secondary.status.borrow(), Status::StorageError);
        if needs_consent && !storage_error {
            for_secondary.consent(false);
        } else {
            for_secondary.dismissed.set(true);
            for_secondary.notify();
        }
    });
    render();
    UpdateBanner {
        widget,
        refresh_i18n: render,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn consent_and_daily_budget_are_explicit() {
        let mut preferences = Preferences::default();
        assert!(!due(&preferences, DAY));
        preferences.automatic = Some(false);
        assert!(!due(&preferences, DAY));
        preferences.automatic = Some(true);
        assert!(due(&preferences, DAY));
        preferences.last_attempt = Some(DAY);
        assert!(!due(&preferences, DAY + 1));
        assert!(!due(&preferences, DAY - 1));
        assert!(due(&preferences, DAY * 2));
    }

    #[test]
    fn corrupt_preferences_fail_closed() {
        assert!(serde_json::from_slice::<Preferences>(b"invalid").is_err());
        assert!(serde_json::from_slice::<Preferences>(br#"{"automatic":"yes"}"#).is_err());
        assert!(
            serde_json::from_slice::<Preferences>(br#"{}"#).is_ok_and(|p| p.automatic.is_none())
        );
    }

    fn drain(context: &glib::MainContext) {
        let until = std::time::Instant::now() + std::time::Duration::from_millis(80);
        while std::time::Instant::now() < until {
            while context.pending() {
                context.iteration(false);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    fn button(widget: &impl IsA<gtk4::Widget>, label: &str) -> Option<gtk4::Button> {
        let widget = widget.as_ref();
        if let Some(button) = widget.downcast_ref::<gtk4::Button>()
            && button.label().is_some_and(|text| text == label)
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if let Some(found) = button(&current, label) {
                return Some(found);
            }
            child = current.next_sibling();
        }
        None
    }

    /// Uses isolated preferences and simulated results, never the user's consent or the network.
    pub(crate) fn native_update_scenario() -> anyhow::Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let path = std::env::temp_dir().join(format!("hv-updates-{}", uuid::Uuid::new_v4()));
        let file = path.join("preferences.json");
        let checker = Rc::new(Checker {
            preferences: RefCell::new(Preferences::default()),
            path: file.clone(),
            runtime: RefCell::new(Some(runtime.handle().clone())),
            status: RefCell::new(Status::Idle),
            observers: RefCell::new(Vec::new()),
            generation: Cell::new(0),
            task: RefCell::new(None),
            dismissed: Cell::new(false),
        });
        let context = glib::MainContext::default();
        let language = heelonvault_core::i18n::current_language();
        assert!(heelonvault_core::i18n::set_language("fr"));
        let banner = build_banner_for(Rc::clone(&checker));
        let preferences = build_preferences_for(Rc::clone(&checker));
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 20);
        content.append(&banner.widget);
        content.append(&preferences.group);
        let window = adw::Window::builder()
            .default_width(760)
            .default_height(530)
            .content(&content)
            .build();
        window.present();
        drain(&context);
        assert!(banner.widget.is_visible());
        // No consent: even with a runtime, the real automatic entry point cannot start a request.
        checker.check(false);
        assert!(checker.task.borrow().is_none());
        assert!(!file.exists());
        if let Some(capture_language) = crate::ui::doc_screenshots::language() {
            assert!(heelonvault_core::i18n::set_language(&capture_language));
            (banner.refresh_i18n)();
            (preferences.refresh_i18n)();
            crate::ui::doc_screenshots::capture(&window, "updates-consent")?;
            assert!(heelonvault_core::i18n::set_language("fr"));
            (banner.refresh_i18n)();
            (preferences.refresh_i18n)();
        }
        button(&banner.widget, &tr("updates-not-now"))
            .ok_or_else(|| anyhow::anyhow!("missing consent dismissal"))?
            .emit_clicked();
        assert!(!banner.widget.is_visible());
        assert_eq!(read_preferences(&file)?.automatic, Some(false));

        let requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = std::sync::Arc::clone(&requests);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        checker.check_with(true, async move {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            receiver.await?;
            check::StableRelease::from_tag("v99.0.0")
        });
        assert!(matches!(*checker.status.borrow(), Status::Checking));
        // A second check while busy must not create another request.
        checker.check_with(true, async { anyhow::bail!("duplicate request") });
        assert!(
            !button(&preferences.group, &tr("updates-check"))
                .ok_or_else(|| anyhow::anyhow!("missing manual check"))?
                .is_sensitive()
        );
        sender
            .send(())
            .map_err(|_| anyhow::anyhow!("worker receiver dropped"))?;
        drain(&context);
        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(matches!(*checker.status.borrow(), Status::Available(_)));
        assert!(banner.widget.is_visible());
        assert_eq!(read_preferences(&file)?.latest.as_deref(), Some("v99.0.0"));
        // Re-enabling during the daily cooldown keeps the known result without a request.
        checker.consent(true);
        assert!(checker.task.borrow().is_none());
        assert!(matches!(*checker.status.borrow(), Status::Available(_)));
        assert_eq!(read_preferences(&file)?.automatic, Some(true));
        crate::ui::doc_screenshots::capture(&window, "updates-available")?;
        assert!(heelonvault_core::i18n::set_language("en"));
        (banner.refresh_i18n)();
        (preferences.refresh_i18n)();
        assert!(button(&banner.widget, "Explore release").is_some());
        assert_eq!(preferences.group.title(), "Application updates");
        assert!(button(&preferences.group, "Check now").is_some());
        crate::ui::doc_screenshots::capture(&window, "updates-available")?;

        if let Some(dir) = std::env::var_os("HEELONVAULT_UI_SNAPSHOT_DIR") {
            drain(&context);
            let paintable = gtk4::WidgetPaintable::new(Some(&window));
            let snapshot = gtk4::Snapshot::new();
            paintable.snapshot(
                &snapshot,
                f64::from(window.width()),
                f64::from(window.height()),
            );
            let node = snapshot
                .to_node()
                .ok_or_else(|| anyhow::anyhow!("empty update snapshot"))?;
            let surface = window
                .surface()
                .ok_or_else(|| anyhow::anyhow!("missing update surface"))?;
            let renderer = gtk4::gsk::Renderer::for_surface(&surface)
                .ok_or_else(|| anyhow::anyhow!("missing update renderer"))?;
            renderer
                .render_texture(&node, None)
                .save_to_png(PathBuf::from(dir).join("updates-available.png"))?;
            renderer.unrealize();
        }
        button(&banner.widget, &tr("updates-later"))
            .ok_or_else(|| anyhow::anyhow!("missing later action"))?
            .emit_clicked();
        assert!(!banner.widget.is_visible());
        // An error must never retain the "up to date" success state.
        checker.check_with(true, async { anyhow::bail!("offline simulation") });
        drain(&context);
        assert!(matches!(*checker.status.borrow(), Status::Unavailable));
        assert!(!banner.widget.is_visible());
        checker.check_with(true, async {
            check::StableRelease::from_tag(env!("CARGO_PKG_VERSION"))
        });
        drain(&context);
        assert!(matches!(*checker.status.borrow(), Status::Current));

        // Revoking consent aborts in-flight work and rejects any late result.
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        checker.check_with(true, async move {
            receiver.await?;
            check::StableRelease::from_tag("v99.0.0")
        });
        checker.consent(false);
        let _ = sender.send(());
        drain(&context);
        assert!(matches!(*checker.status.borrow(), Status::Idle));
        assert!(checker.task.borrow().is_none());
        checker.check(false);
        assert!(checker.task.borrow().is_none());

        // A path whose parent is a file forces a portable persistence failure.
        std::fs::remove_file(&file)?;
        std::fs::remove_dir(&path)?;
        std::fs::write(&path, b"not a directory")?;
        checker.consent(true);
        assert!(matches!(*checker.status.borrow(), Status::StorageError));
        assert_eq!(checker.preferences.borrow().automatic, Some(false));
        assert!(checker.task.borrow().is_none());
        assert!(banner.widget.is_visible());
        assert!(read_preferences(&file).is_err());
        checker.dismissed.set(true);
        checker.consent(false);
        assert!(banner.widget.is_visible());
        std::fs::remove_file(&path)?;
        window.close();
        drain(&context);
        assert!(heelonvault_core::i18n::set_language(&language));
        Ok(())
    }
}

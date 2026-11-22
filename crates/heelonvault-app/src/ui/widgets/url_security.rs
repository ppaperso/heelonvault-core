//! URL safety indicator of the secret editor.
//!
//! A single compact row under the URL entry: a badge for the offline assessment,
//! a badge for the optional DNS/TLS check (shown only once requested), a small
//! "Check" button and an info popover holding the explanatory texts. Long
//! messages live in tooltips and the popover, so the form never grows extra
//! paragraphs. Everything is rendered from one state, which keeps live
//! retranslation and cancellation consistent.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk4::{glib, prelude::*};
use heelonvault_core::i18n::{I18nArg, tr, tr_args};
use heelonvault_core::services::security_center::{UrlStatus, assess_url};
use heelonvault_url_check::CheckError;
use libadwaita::{self as adw, prelude::*};
use tokio::runtime::Handle;

pub(crate) fn url_status_key(status: UrlStatus) -> &'static str {
    match status {
        UrlStatus::Empty => "security-url-empty",
        UrlStatus::Invalid => "security-url-invalid",
        UrlStatus::UnsupportedScheme => "security-url-scheme",
        UrlStatus::Credentials => "security-url-credentials",
        UrlStatus::Http => "security-url-http",
        UrlStatus::LocalHost => "security-url-local",
        UrlStatus::InternationalDomain => "security-url-international",
        _ => "security-url-unverified",
    }
}

/// A completed online check: its result and the local time it finished.
type Remembered = (Result<(), CheckError>, String);

thread_local! {
    /// Completed checks of the current session, keyed by checked origin. Kept in
    /// memory only (a TLS result is a point-in-time fact, never persisted) and
    /// wiped on lock and logout by [`forget_session_checks`]. GTK main thread only.
    static SESSION_CHECKS: RefCell<HashMap<String, Remembered>> = RefCell::new(HashMap::new());
}

/// Forgets every remembered check; called by every lock and logout path.
pub(crate) fn forget_session_checks() {
    SESSION_CHECKS.with(|checks| checks.borrow_mut().clear());
}

fn remembered(origin: &str) -> Option<Remembered> {
    SESSION_CHECKS.with(|checks| checks.borrow().get(origin).cloned())
}

fn remember(origin: String, check: Remembered) {
    SESSION_CHECKS.with(|checks| checks.borrow_mut().insert(origin, check));
}

/// What an online check contacts: scheme, host and port only, never the path.
/// Same origin means the remembered result still describes this URL.
fn checked_origin(raw: &str) -> Option<String> {
    if !checkable(assess_url(raw)) {
        return None;
    }
    // Same trimming as `assess_url`.
    url::Url::parse(raw.trim())
        .ok()
        .map(|url| url.origin().ascii_serialization())
}

/// Severity of a badge; never claims more than the check established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Neutral,
    Ok,
    Warning,
    Error,
}

impl Tone {
    const CLASSES: [&'static str; 4] = [
        "url-badge-neutral",
        "url-badge-ok",
        "url-badge-warning",
        "url-badge-error",
    ];

    fn class(self) -> &'static str {
        match self {
            Self::Neutral => Self::CLASSES[0],
            Self::Ok => Self::CLASSES[1],
            Self::Warning => Self::CLASSES[2],
            Self::Error => Self::CLASSES[3],
        }
    }
}

/// How a badge looks: icon, short label key, long (tooltip) key and tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Look {
    icon: &'static str,
    short: &'static str,
    long: &'static str,
    tone: Tone,
}

fn local_look(status: UrlStatus) -> Look {
    let (icon, short, tone) = match status {
        UrlStatus::HttpsUnverified => ("hv-lock-symbolic", "url-badge-https", Tone::Neutral),
        UrlStatus::Http => (
            "hv-shield-warning-symbolic",
            "url-badge-http",
            Tone::Warning,
        ),
        UrlStatus::LocalHost => (
            "hv-shield-warning-symbolic",
            "url-badge-local",
            Tone::Warning,
        ),
        UrlStatus::InternationalDomain => (
            "hv-shield-warning-symbolic",
            "url-badge-international",
            Tone::Warning,
        ),
        UrlStatus::Invalid => ("hv-warning-symbolic", "url-badge-invalid", Tone::Error),
        UrlStatus::UnsupportedScheme => ("hv-warning-symbolic", "url-badge-scheme", Tone::Error),
        UrlStatus::Credentials => ("hv-warning-symbolic", "url-badge-credentials", Tone::Error),
        _ => (
            "hv-shield-partial-symbolic",
            "url-badge-unchecked",
            Tone::Neutral,
        ),
    };
    Look {
        icon,
        short,
        long: url_status_key(status),
        tone,
    }
}

/// An unreachable network is "unknown", never reported as a threat.
fn network_look(result: Result<(), CheckError>) -> Look {
    let (long, short, icon, tone) = match result {
        Ok(()) => (
            "security-url-tls-ok",
            "url-badge-tls-ok",
            "hv-shield-ok-symbolic",
            Tone::Ok,
        ),
        Err(CheckError::Tls) => (
            "security-url-tls-error",
            "url-badge-tls-error",
            "hv-warning-symbolic",
            Tone::Error,
        ),
        Err(CheckError::NonPublicAddress) => (
            "security-url-blocked",
            "url-badge-blocked",
            "hv-shield-warning-symbolic",
            Tone::Warning,
        ),
        Err(CheckError::Timeout) => (
            "security-url-timeout",
            "url-badge-unknown",
            "hv-shield-partial-symbolic",
            Tone::Neutral,
        ),
        Err(CheckError::Resolution) => (
            "security-url-dns-error",
            "url-badge-unknown",
            "hv-shield-partial-symbolic",
            Tone::Neutral,
        ),
        Err(CheckError::Connection) => (
            "security-url-connection-error",
            "url-badge-unknown",
            "hv-shield-partial-symbolic",
            Tone::Neutral,
        ),
        _ => (
            "security-url-check-error",
            "url-badge-unknown",
            "hv-shield-partial-symbolic",
            Tone::Warning,
        ),
    };
    Look {
        icon,
        short,
        long,
        tone,
    }
}

/// Only HTTPS hosts that are not internal can be checked online.
fn checkable(status: UrlStatus) -> bool {
    matches!(
        status,
        UrlStatus::HttpsUnverified | UrlStatus::InternationalDomain
    )
}

/// Pill made of an icon (or a spinner while busy) and a short label.
struct Badge {
    root: gtk4::Box,
    icon: gtk4::Image,
    spinner: gtk4::Spinner,
    label: gtk4::Label,
}

impl Badge {
    fn new(name: &str) -> Self {
        let root = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        root.add_css_class("url-badge");
        root.add_css_class(name);
        root.set_valign(gtk4::Align::Center);
        let icon = gtk4::Image::new();
        icon.set_pixel_size(14);
        let spinner = gtk4::Spinner::new();
        spinner.set_visible(false);
        let label = gtk4::Label::new(None);
        label.set_single_line_mode(true);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        root.append(&icon);
        root.append(&spinner);
        root.append(&label);
        Self {
            root,
            icon,
            spinner,
            label,
        }
    }

    fn tone(&self, tone: Tone) {
        for class in Tone::CLASSES {
            self.root.remove_css_class(class);
        }
        self.root.add_css_class(tone.class());
    }

    fn show(&self, look: Look, tooltip: &str) {
        self.spinner.stop();
        self.spinner.set_visible(false);
        self.icon.set_icon_name(Some(look.icon));
        self.icon.set_visible(true);
        self.label.set_text(&tr(look.short));
        self.root.set_tooltip_text(Some(tooltip));
        self.tone(look.tone);
        self.root.set_visible(true);
    }

    fn busy(&self, label: &str, tooltip: &str) {
        self.icon.set_visible(false);
        self.spinner.set_visible(true);
        self.spinner.start();
        self.label.set_text(label);
        self.root.set_tooltip_text(Some(tooltip));
        self.tone(Tone::Neutral);
        self.root.set_visible(true);
    }

    fn hide(&self) {
        self.spinner.stop();
        self.root.set_visible(false);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Network {
    Off,
    Checking,
    Done(Result<(), CheckError>),
}

struct State {
    generation: Cell<u64>,
    task: RefCell<Option<tokio::task::AbortHandle>>,
    status: Cell<UrlStatus>,
    origin: RefCell<Option<String>>,
    network: Cell<Network>,
    checked_at: RefCell<String>,
    bar: gtk4::Box,
    local: Badge,
    remote: Badge,
    check: gtk4::Button,
    check_label: gtk4::Label,
    details: gtk4::MenuButton,
    details_text: gtk4::Label,
}

impl State {
    fn network_text(&self) -> String {
        match self.network.get() {
            Network::Off => tr("security-url-network-off"),
            Network::Checking => tr("security-url-checking"),
            Network::Done(result) => tr_args(
                "security-url-result",
                &[
                    ("result", I18nArg::Str(&tr(network_look(result).long))),
                    ("time", I18nArg::Str(self.checked_at.borrow().as_str())),
                ],
            ),
        }
    }

    /// Single source of truth for every visible text, icon and class.
    fn render(&self) {
        let status = self.status.get();
        let look = local_look(status);
        let local_text = tr(look.long);
        let network_text = self.network_text();

        self.check_label.set_text(&tr("url-check-short"));
        self.check.set_tooltip_text(Some(&tr("security-url-check")));
        self.details.set_tooltip_text(Some(&tr("url-details")));
        self.details_text.set_text(&format!(
            "{local_text}\n\n{network_text}\n\n{}",
            tr("security-url-reputation-off")
        ));

        if status == UrlStatus::Empty {
            self.bar.set_visible(false);
            return;
        }
        self.bar.set_visible(true);
        self.local.show(look, &local_text);

        match self.network.get() {
            Network::Off => self.remote.hide(),
            Network::Checking => self.remote.busy(&tr("url-badge-checking"), &network_text),
            Network::Done(result) => self.remote.show(network_look(result), &network_text),
        }
        self.check.set_visible(checkable(status));
        self.check
            .set_sensitive(self.network.get() != Network::Checking);
    }

    /// Changing the value or leaving the editor aborts any running check.
    fn cancel(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.network.set(Network::Off);
    }

    /// Shows the result already obtained for this origin during the session, if any.
    fn restore(&self) {
        let origin = self.origin.borrow();
        if let Some((result, time)) = origin.as_deref().and_then(remembered) {
            *self.checked_at.borrow_mut() = time;
            self.network.set(Network::Done(result));
        }
    }

    /// A new value starts from a clean bar, unless its origin was already checked.
    fn assess(&self, raw: &str) {
        self.cancel();
        self.status.set(assess_url(raw));
        *self.origin.borrow_mut() = checked_origin(raw);
        self.restore();
        self.render();
    }
}

/// Live-retranslation handle of the indicator, held by the editor state.
pub(crate) struct UrlCheck(Weak<State>);

impl UrlCheck {
    pub(crate) fn refresh_i18n(&self) {
        if let Some(state) = self.0.upgrade() {
            state.render();
        }
    }
}

fn build_details() -> (gtk4::MenuButton, gtk4::Label) {
    let text = gtk4::Label::new(None);
    text.set_wrap(true);
    text.set_max_width_chars(46);
    text.set_xalign(0.0);
    text.add_css_class("url-details-text");
    let popover = gtk4::Popover::new();
    popover.set_child(Some(&text));
    let button = gtk4::MenuButton::new();
    button.set_icon_name("hv-info-symbolic");
    button.set_popover(Some(&popover));
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.add_css_class("url-details-button");
    button.set_valign(gtk4::Align::Center);
    (button, text)
}

fn build_check_button() -> (gtk4::Button, gtk4::Label) {
    let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let icon = gtk4::Image::from_icon_name("hv-shield-symbolic");
    icon.set_pixel_size(14);
    let label = gtk4::Label::new(None);
    content.append(&icon);
    content.append(&label);
    let button = gtk4::Button::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
    button.add_css_class("url-check-button");
    button.set_valign(gtk4::Align::Center);
    (button, label)
}

/// Each explicit check gets its own consent; changing the value or leaving the editor aborts it.
pub(crate) fn attach(entry: &gtk4::Entry, row: &gtk4::Box, runtime: Handle) -> UrlCheck {
    let bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    bar.add_css_class("url-check-bar");
    let local = Badge::new("url-badge-local");
    let remote = Badge::new("url-badge-remote");
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let (check, check_label) = build_check_button();
    let (details, details_text) = build_details();
    bar.append(&local.root);
    bar.append(&remote.root);
    bar.append(&spacer);
    bar.append(&check);
    bar.append(&details);
    row.append(&bar);

    let state = Rc::new(State {
        generation: Cell::new(0),
        task: RefCell::new(None),
        status: Cell::new(UrlStatus::Empty),
        origin: RefCell::new(None),
        network: Cell::new(Network::Off),
        checked_at: RefCell::new(String::new()),
        bar,
        local,
        remote,
        check,
        check_label,
        details,
        details_text,
    });
    state.assess(entry.text().as_str());

    let changed = Rc::downgrade(&state);
    entry.connect_changed(move |entry| {
        if let Some(state) = changed.upgrade() {
            state.assess(entry.text().as_str());
        }
    });
    let unmapped = Rc::downgrade(&state);
    row.connect_unmap(move |_| {
        if let Some(state) = unmapped.upgrade() {
            // Only a running check is dropped; a completed one stays remembered.
            state.cancel();
            state.restore();
            state.render();
        }
    });

    let weak = Rc::downgrade(&state);
    let entry_ref = entry.downgrade();
    state.check.connect_clicked(move |_| {
        let (Some(state), Some(entry)) = (weak.upgrade(), entry_ref.upgrade()) else {
            return;
        };
        request_check(&state, &entry, runtime.clone());
    });

    // Signal handlers hold weak references; the row keeps the state alive.
    let keep = Rc::clone(&state);
    row.connect_destroy(move |_| keep.cancel());
    UrlCheck(Rc::downgrade(&state))
}

fn request_check(state: &Rc<State>, entry: &gtk4::Entry, runtime: Handle) {
    // The button is only visible for a checkable value.
    let Some(origin) = checked_origin(entry.text().as_str()) else {
        return;
    };
    let Some(window) = entry.root().and_downcast::<gtk4::Window>() else {
        return;
    };
    let dialog = adw::MessageDialog::new(
        Some(&window),
        Some(&tr("security-url-consent-title")),
        Some(&tr_args(
            "security-url-consent-body",
            &[("origin", I18nArg::Str(&origin))],
        )),
    );
    dialog.add_response("cancel", &tr("security-center-cancel"));
    dialog.add_response("check", &tr("security-url-check"));
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let generation = state.generation.get();
    let weak = Rc::downgrade(state);
    let entry = entry.downgrade();
    dialog.connect_response(None, move |_, response| {
        let (Some(state), Some(entry)) = (weak.upgrade(), entry.upgrade()) else {
            return;
        };
        if response != "check" || generation != state.generation.get() || !entry.is_mapped() {
            return;
        }
        state.network.set(Network::Checking);
        state.render();
        let target = origin.clone();
        let task = runtime.spawn(async move { heelonvault_url_check::check_https(&target).await });
        *state.task.borrow_mut() = Some(task.abort_handle());
        let weak = Rc::downgrade(&state);
        let entry = entry.downgrade();
        let checked = origin.clone();
        glib::MainContext::default().spawn_local(async move {
            let result = task.await;
            let (Some(state), Some(entry)) = (weak.upgrade(), entry.upgrade()) else {
                return;
            };
            if generation != state.generation.get() || !entry.is_mapped() {
                return;
            }
            state.task.borrow_mut().take();
            let time = crate::ui::local_time::now_formatted("%H:%M");
            // A panicked task is reported as "unknown state", like an unreachable host.
            let result = result.unwrap_or(Err(CheckError::Connection));
            remember(checked, (result, time.clone()));
            *state.checked_at.borrow_mut() = time;
            state.network.set(Network::Done(result));
            state.render();
        });
    });
    dialog.present();
}

/// Native GTK check of the indicator (run from `native_gtk_smoke`).
#[cfg(test)]
pub(crate) fn native_url_check_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let row = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let entry = gtk4::Entry::new();
    row.append(&entry);
    let check = attach(&entry, &row, runtime.handle().clone());
    let state = check.0.upgrade().ok_or("indicator state dropped")?;
    let language = heelonvault_core::i18n::current_language();

    // No URL: no row at all, so the form stays compact.
    assert!(!state.bar.is_visible());

    entry.set_text("http://example.com");
    assert!(state.bar.is_visible());
    assert_eq!(state.local.label.text(), tr("url-badge-http"));
    assert!(state.local.root.has_css_class("url-badge-warning"));
    assert!(
        !state.check.is_visible(),
        "plain HTTP is never checked online"
    );
    assert!(!state.remote.root.is_visible());

    entry.set_text("https://example.com/login");
    assert_eq!(state.local.label.text(), tr("url-badge-https"));
    assert!(state.local.root.has_css_class("url-badge-neutral"));
    assert!(state.check.is_visible() && state.check.is_sensitive());
    assert_eq!(
        state.local.root.tooltip_text().as_deref(),
        Some(tr("security-url-unverified").as_str())
    );

    // A finished check shows a second badge; editing the URL discards it.
    state.network.set(Network::Done(Ok(())));
    state.render();
    assert!(state.remote.root.is_visible());
    assert!(state.remote.root.has_css_class("url-badge-ok"));
    entry.set_text("https://example.org");
    assert_eq!(state.network.get(), Network::Off);
    assert!(!state.remote.root.is_visible());

    for lang in ["fr", "en"] {
        assert!(heelonvault_core::i18n::set_language(lang));
        check.refresh_i18n();
        assert_eq!(state.check_label.text(), tr("url-check-short"));
        assert_eq!(state.local.label.text(), tr("url-badge-https"));
        assert!(
            state
                .details_text
                .text()
                .contains(&tr("security-url-reputation-off"))
        );
    }
    assert!(heelonvault_core::i18n::set_language(&language));

    entry.set_text("");
    assert!(!state.bar.is_visible());

    // Session memory: coming back to a secret shows the result already obtained
    // for its host; another host starts from a clean bar; locking forgets it.
    forget_session_checks();
    remember(
        "https://accounts.example.com".into(),
        (Ok(()), "14:32".into()),
    );
    let row = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let entry = gtk4::Entry::new();
    entry.set_text("https://accounts.example.com/login");
    row.append(&entry);
    let reopened = attach(&entry, &row, runtime.handle().clone());
    let state = reopened.0.upgrade().ok_or("indicator state dropped")?;
    assert_eq!(state.network.get(), Network::Done(Ok(())));
    assert!(state.remote.root.is_visible());
    assert!(
        state
            .remote
            .root
            .tooltip_text()
            .is_some_and(|tooltip| tooltip.contains("14:32"))
    );
    entry.set_text("https://accounts.example.com/signin");
    assert!(state.remote.root.is_visible(), "same host keeps its result");
    entry.set_text("https://account.example.com/signin");
    assert!(!state.remote.root.is_visible(), "another host starts over");
    assert_eq!(state.network.get(), Network::Off);
    entry.set_text("https://accounts.example.com/login");
    assert!(state.remote.root.is_visible());
    row.unmap();
    assert!(
        state.remote.root.is_visible(),
        "leaving the editor keeps it"
    );
    forget_session_checks();
    entry.set_text("https://accounts.example.com/");
    assert!(
        !state.remote.root.is_visible(),
        "locking forgets every result"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_network_is_not_reported_as_malicious() {
        for error in [
            CheckError::Resolution,
            CheckError::Timeout,
            CheckError::Connection,
        ] {
            let look = network_look(Err(error));
            assert_eq!(look.tone, Tone::Neutral);
            assert_eq!(look.short, "url-badge-unknown");
        }
        assert_eq!(network_look(Err(CheckError::Tls)).tone, Tone::Error);
        assert_eq!(network_look(Ok(())).long, "security-url-tls-ok");
        assert_eq!(network_look(Ok(())).tone, Tone::Ok);
    }

    #[test]
    fn https_is_neutral_not_a_trust_claim() {
        let look = local_look(UrlStatus::HttpsUnverified);
        assert_eq!(look.tone, Tone::Neutral);
        assert_eq!(look.long, "security-url-unverified");
        assert!(checkable(UrlStatus::HttpsUnverified));
    }

    #[test]
    fn risky_urls_are_flagged_and_not_checkable_online() {
        for status in [
            UrlStatus::Invalid,
            UrlStatus::UnsupportedScheme,
            UrlStatus::Credentials,
        ] {
            assert_eq!(local_look(status).tone, Tone::Error);
            assert!(!checkable(status));
        }
        for status in [UrlStatus::Http, UrlStatus::LocalHost] {
            assert_eq!(local_look(status).tone, Tone::Warning);
            assert!(!checkable(status));
        }
        assert_eq!(
            local_look(UrlStatus::InternationalDomain).tone,
            Tone::Warning
        );
        assert!(checkable(UrlStatus::InternationalDomain));
    }

    #[test]
    fn remembered_results_follow_the_checked_origin_not_the_path() {
        let origin = checked_origin("https://accounts.example.com/login");
        assert_eq!(origin.as_deref(), Some("https://accounts.example.com"));
        assert_eq!(
            checked_origin(" https://accounts.example.com/signin?x=1 "),
            origin
        );
        assert_ne!(checked_origin("https://account.example.com/login"), origin);
        assert_ne!(checked_origin("https://accounts.example.com:8443/"), origin);
        // Values that are never checked online have no origin to remember.
        assert_eq!(checked_origin("http://accounts.example.com/login"), None);
        assert_eq!(checked_origin("https://192.168.1.10/"), None);
        assert_eq!(checked_origin(""), None);
    }

    #[test]
    fn every_badge_key_exists_in_both_catalogs() {
        let mut keys = vec!["url-check-short", "url-details", "url-badge-checking"];
        for status in [
            UrlStatus::Empty,
            UrlStatus::Invalid,
            UrlStatus::UnsupportedScheme,
            UrlStatus::Credentials,
            UrlStatus::Http,
            UrlStatus::LocalHost,
            UrlStatus::InternationalDomain,
            UrlStatus::HttpsUnverified,
        ] {
            keys.push(local_look(status).short);
            keys.push(local_look(status).long);
        }
        for result in [
            Ok(()),
            Err(CheckError::Tls),
            Err(CheckError::NonPublicAddress),
            Err(CheckError::Timeout),
            Err(CheckError::Resolution),
            Err(CheckError::Connection),
            Err(CheckError::InvalidUrl),
        ] {
            keys.push(network_look(result).short);
            keys.push(network_look(result).long);
        }
        for lang in ["fr", "en"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../heelonvault-core/locales")
                .join(lang)
                .join("main.ftl");
            let catalog = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(!catalog.is_empty(), "cannot read {}", path.display());
            for key in &keys {
                let defined = catalog
                    .lines()
                    .any(|line| line.split_once(" =").is_some_and(|(id, _)| id == *key));
                assert!(defined, "{key} missing in {lang}");
            }
        }
    }
}

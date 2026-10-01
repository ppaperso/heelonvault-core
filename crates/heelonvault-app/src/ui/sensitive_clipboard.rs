//! Copies secrets to the system clipboard and takes them back.
//!
//! A copied secret stays readable by every other application until something replaces it, and
//! GNOME cannot simply be asked to replace it later: Mutter refuses a clipboard change from a
//! window without keyboard focus — which is where the user is when the delay expires, pasting in
//! a browser. Mutter also snapshots the clipboard as soon as it changes owner and republishes
//! that snapshot when the owner quits, so the password came back even after HeelonVault exited.
//!
//! What is done instead, none of which needs focus:
//! - HeelonVault stays the owner of the clipboard and serves the secret itself, through
//!   [`SecretContent`]. At expiry the provider drops (and zeroes) the secret: every later paste
//!   receives an empty text, whatever window has focus.
//! - Reads that arrive within [`SNAPSHOT_WINDOW`] of the copy receive an empty text: only the
//!   compositor's snapshot and clipboard-history tools read that fast (Mutter reads within
//!   ~3 ms), never a human paste. The snapshot republished after exit is therefore empty.
//! - The content is flagged `x-kde-passwordManagerHint: secret` and offers no storable format,
//!   for history managers that honour either.
//! - When a HeelonVault window has focus, the clipboard is also emptied for good.
//!
//! Whatever HeelonVault exposes this way is published as an [`Exposure`] to subscribers (the
//! header indicator): a copy being decrypted, then the sensitive value served by the clipboard
//! until it expires, is cleared, or is replaced by another application's copy. Nothing else is
//! claimed — the master key of an open session or OS clipboard history are out of its reach.
//!
//! All functions run on the GTK main thread.

use std::cell::RefCell;
#[cfg(not(windows))]
use std::future::Future;
#[cfg(not(windows))]
use std::pin::Pin;
use std::time::Duration;
use std::time::Instant;

use gtk4::gdk;
#[cfg(not(windows))]
use gtk4::gdk::subclass::prelude::*;
#[cfg(not(windows))]
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
#[cfg(not(windows))]
use zeroize::{Zeroize, Zeroizing};

/// Password or login copied from a vault entry.
pub const SECRET_CLEAR_DELAY: Duration = Duration::from_secs(20);
/// Recovery phrase: the user needs time to paste it into a password manager or a document.
pub const RECOVERY_PHRASE_CLEAR_DELAY: Duration = Duration::from_secs(60);
/// Reads this soon after a copy come from the compositor or a history tool, not from a paste.
/// Only meaningful for the on-demand-serving provider used on non-Windows — see module docs.
#[cfg(any(test, not(windows)))]
const SNAPSHOT_WINDOW: Duration = Duration::from_millis(150);

const PASSWORD_MANAGER_HINT_MIME: &str = "x-kde-passwordManagerHint";
const TEXT_MIME_TYPES: [&str; 2] = ["text/plain;charset=utf-8", "text/plain"];

/// Whether a read arriving `since_copy` after the copy may receive the secret.
#[cfg(any(test, not(windows)))]
fn serves_secret(since_copy: Duration, expired: bool) -> bool {
    !expired && since_copy >= SNAPSHOT_WINDOW
}

/// What kind of value a sensitive copy holds (shown by the header indicator).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensitiveKind {
    Password,
    Login,
    RecoveryPhrase,
}

/// What HeelonVault itself currently exposes through the clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exposure {
    /// No sensitive value is being decrypted for a copy nor served by the clipboard.
    Idle,
    /// A secret is being decrypted on demand for a copy (a few milliseconds).
    Decrypting,
    /// A sensitive value is served by the clipboard until `expires_at`.
    InClipboard {
        kind: SensitiveKind,
        copied_at: Instant,
        expires_at: Instant,
    },
}

/// A copy currently served by the clipboard.
#[derive(Clone, Copy, Debug)]
struct ActiveCopy {
    kind: SensitiveKind,
    copied_at: Instant,
    expires_at: Instant,
}

/// The clipboard copy wins over a decryption in flight: it is the longer-lived exposure.
fn exposure_of(active: Option<ActiveCopy>, decrypting: u32) -> Exposure {
    match active {
        Some(copy) => Exposure::InClipboard {
            kind: copy.kind,
            copied_at: copy.copied_at,
            expires_at: copy.expires_at,
        },
        None if decrypting > 0 => Exposure::Decrypting,
        None => Exposure::Idle,
    }
}

/// Which copy the clipboard may still hold. Kept free of GTK so the policy is unit-tested.
#[derive(Debug, Default)]
struct Ledger {
    generation: u64,
    pending: bool,
}

impl Ledger {
    fn record_copy(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending = true;
        self.generation
    }

    /// The timer of copy `generation` fired: empty the clipboard only if it is still the
    /// latest sensitive copy and the clipboard is still owned by this process.
    fn expire(&mut self, generation: u64, clipboard_is_ours: bool) -> bool {
        if !self.pending || generation != self.generation {
            return false;
        }
        self.pending = false;
        clipboard_is_ours
    }

    /// Lock, logout or exit: empty the clipboard if a sensitive copy may still be there.
    fn take_pending(&mut self, clipboard_is_ours: bool) -> bool {
        let was_pending = self.pending;
        self.pending = false;
        was_pending && clipboard_is_ours
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SecretContent {
        pub(super) secret: RefCell<Option<Zeroizing<String>>>,
        pub(super) copied_at: RefCell<Option<Instant>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SecretContent {
        const NAME: &'static str = "HeelonVaultSecretContent";
        type Type = super::SecretContent;
        type ParentType = gdk::ContentProvider;
    }

    impl ObjectImpl for SecretContent {}

    impl ContentProviderImpl for SecretContent {
        fn formats(&self) -> gdk::ContentFormats {
            gdk::ContentFormatsBuilder::new()
                .add_type(glib::Type::STRING)
                .add_mime_type(TEXT_MIME_TYPES[0])
                .add_mime_type(TEXT_MIME_TYPES[1])
                .add_mime_type(PASSWORD_MANAGER_HINT_MIME)
                .build()
        }

        // X11 clipboard managers only save the formats listed here when the owner exits.
        fn storable_formats(&self) -> gdk::ContentFormats {
            gdk::ContentFormatsBuilder::new().build()
        }

        fn write_mime_type_future(
            &self,
            mime_type: &str,
            stream: &gio::OutputStream,
            io_priority: glib::Priority,
        ) -> Pin<Box<dyn Future<Output = Result<(), glib::Error>> + 'static>> {
            let bytes = if mime_type == PASSWORD_MANAGER_HINT_MIME {
                b"secret".to_vec()
            } else if TEXT_MIME_TYPES.contains(&mime_type) {
                self.current_text().into_bytes()
            } else {
                let error = glib::Error::new(
                    gio::IOErrorEnum::NotSupported,
                    "unsupported clipboard format",
                );
                return Box::pin(async move { Err(error) });
            };
            let stream = stream.clone();
            Box::pin(async move {
                let written = stream.write_all_future(bytes, io_priority).await;
                let (mut buffer, outcome) = match written {
                    Ok((buffer, _, None)) => (buffer, Ok(())),
                    Ok((buffer, _, Some(error))) | Err((buffer, error)) => (buffer, Err(error)),
                };
                buffer.zeroize();
                outcome?;
                stream.close_future(io_priority).await
            })
        }

        fn value(&self, type_: glib::Type) -> Result<glib::Value, glib::Error> {
            if type_ == glib::Type::STRING {
                return Ok(self.current_text().to_value());
            }
            self.parent_value(type_)
        }
    }

    impl SecretContent {
        fn current_text(&self) -> String {
            let since_copy = self
                .copied_at
                .borrow()
                .map_or(Duration::ZERO, |copied_at| copied_at.elapsed());
            let secret = self.secret.borrow();
            match secret.as_ref() {
                Some(text) if serves_secret(since_copy, false) => text.as_str().to_string(),
                _ => String::new(),
            }
        }
    }
}

#[cfg(not(windows))]
glib::wrapper! {
    /// Clipboard content that serves a secret until it expires, then an empty text.
    pub struct SecretContent(ObjectSubclass<imp::SecretContent>)
        @extends gdk::ContentProvider;
}

#[cfg(not(windows))]
impl SecretContent {
    fn new(text: &str) -> Self {
        let content: Self = glib::Object::new();
        *content.imp().secret.borrow_mut() = Some(Zeroizing::new(text.to_string()));
        *content.imp().copied_at.borrow_mut() = Some(Instant::now());
        content
    }

    /// Drops the secret (zeroed on drop): every later read receives an empty text.
    fn expire(&self) {
        self.imp().secret.borrow_mut().take();
    }
}

// GDK's Windows clipboard backend (OLE/IDataObject, not X11 selection ownership) faults with
// STATUS_ACCESS_VIOLATION inside libgtk-4-1.dll when content is served through this module's
// custom async ContentProvider (write_mime_type_future) — reproduced consistently on Windows 11,
// same fault offset every time. See SECURITY.md/SECURITY.fr.md. Windows falls back to GDK's
// built-in eager providers (for_bytes/for_value/new_union) instead: well-tested, boring, but
// unlike SecretContent they cannot self-clear on a later read — the whole secret is copied into
// GDK-owned memory up front. expire_provider is a no-op there; wipe_owned_clipboard compensates
// by unconditionally overwriting the clipboard content at expiry instead.

#[cfg(not(windows))]
fn make_content(text: &str) -> gdk::ContentProvider {
    SecretContent::new(text).upcast()
}

#[cfg(windows)]
fn make_content(text: &str) -> gdk::ContentProvider {
    let mut providers: Vec<gdk::ContentProvider> = TEXT_MIME_TYPES
        .iter()
        .map(|mime| {
            gdk::ContentProvider::for_bytes(
                mime,
                &glib::Bytes::from_owned(text.as_bytes().to_vec()),
            )
        })
        .collect();
    providers.push(gdk::ContentProvider::for_value(&text.to_value()));
    providers.push(gdk::ContentProvider::for_bytes(
        PASSWORD_MANAGER_HINT_MIME,
        &glib::Bytes::from_static(b"secret"),
    ));
    gdk::ContentProvider::new_union(&providers)
}

#[cfg(not(windows))]
fn expire_provider(provider: &gdk::ContentProvider) {
    if let Some(secret) = provider.downcast_ref::<SecretContent>() {
        secret.expire();
    }
}

#[cfg(windows)]
fn expire_provider(_provider: &gdk::ContentProvider) {}

#[derive(Default)]
struct State {
    ledger: Ledger,
    timer: Option<glib::SourceId>,
    content: Option<gdk::ContentProvider>,
    active: Option<ActiveCopy>,
    decrypting: u32,
    /// `changed` handler on the display clipboard, connected once on first copy.
    watching_clipboard: bool,
}

/// Called with every new [`Exposure`]; returning `Break` unsubscribes.
type ExposureListener = Box<dyn Fn(Exposure) -> glib::ControlFlow>;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static LISTENERS: RefCell<Vec<ExposureListener>> = const { RefCell::new(Vec::new()) };
}

/// What HeelonVault exposes right now.
pub fn current_exposure() -> Exposure {
    STATE.with(|cell| {
        let state = cell.borrow();
        exposure_of(state.active, state.decrypting)
    })
}

/// Subscribe to exposure changes. `listener` is called at once with the current exposure,
/// then on every change, until it returns `Break` (e.g. its widget is gone).
pub fn subscribe(listener: impl Fn(Exposure) -> glib::ControlFlow + 'static) {
    if listener(current_exposure()).is_break() {
        return;
    }
    LISTENERS.with(|cell| cell.borrow_mut().push(Box::new(listener)));
}

/// Publish the current exposure. Must be called with `STATE` released: listeners may call back
/// into this module (e.g. `clear_now`), and may subscribe while being notified.
fn notify() {
    let exposure = current_exposure();
    let mut listeners = LISTENERS.with(|cell| std::mem::take(&mut *cell.borrow_mut()));
    listeners.retain(|listener| listener(exposure).is_continue());
    LISTENERS.with(|cell| {
        let mut current = cell.borrow_mut();
        // Keep subscriptions made during the notification, after the existing ones.
        listeners.append(&mut current);
        *current = listeners;
    });
}

/// Marks a secret as being decrypted for a copy until dropped (see [`begin_decrypting`]).
#[must_use = "the decryption is reported as running until the guard is dropped"]
pub struct DecryptingGuard {
    _private: (),
}

/// Report that a secret is being decrypted on demand for a copy.
pub fn begin_decrypting() -> DecryptingGuard {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        state.decrypting = state.decrypting.saturating_add(1);
    });
    notify();
    DecryptingGuard { _private: () }
}

impl Drop for DecryptingGuard {
    fn drop(&mut self) {
        STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            state.decrypting = state.decrypting.saturating_sub(1);
        });
        notify();
    }
}

/// Another application replaced the clipboard: our value cannot be pasted anymore, so wipe it
/// now instead of at expiry, and leave the new content alone.
fn watch_clipboard(clipboard: &gdk::Clipboard) {
    let already_watching = STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        std::mem::replace(&mut state.watching_clipboard, true)
    });
    if already_watching {
        return;
    }
    clipboard.connect_changed(|clipboard| {
        if clipboard.is_local() {
            return;
        }
        let released = STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            if state.active.is_none() {
                return false;
            }
            cancel_timer(&mut state);
            expire_content(&mut state);
            state.active = None;
            // Not ours anymore: never wipe it (lock, logout and exit included).
            state.ledger.take_pending(false);
            true
        });
        if released {
            notify();
        }
    });
}

fn system_clipboard() -> Option<gdk::Clipboard> {
    gdk::Display::default().map(|display| display.clipboard())
}

fn cancel_timer(state: &mut State) {
    if let Some(timer) = state.timer.take() {
        timer.remove();
    }
}

fn expire_content(state: &mut State) {
    if let Some(content) = state.content.take() {
        expire_provider(&content);
    }
}

/// Mutter only accepts a clipboard change from the focused window.
#[cfg(not(windows))]
fn application_has_focus() -> bool {
    gtk4::Window::list_toplevels()
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk4::Window>().ok())
        .any(|window| window.is_active())
}

#[cfg(not(windows))]
fn wipe_owned_clipboard(clipboard: &gdk::Clipboard) {
    if application_has_focus() {
        clipboard.set_text("");
        clipboard.display().flush();
    }
}

/// Windows clipboard writes aren't gated on window focus the way Mutter's are (see module doc)
/// — always wipe, compensating for the eager provider's inability to self-clear on read.
#[cfg(windows)]
fn wipe_owned_clipboard(clipboard: &gdk::Clipboard) {
    clipboard.set_text("");
    clipboard.display().flush();
}

/// Puts `text` in the clipboard and schedules its expiry. Returns `false` when nothing was
/// copied.
pub fn copy_sensitive(text: &str, kind: SensitiveKind, clear_after: Duration) -> bool {
    let Some(clipboard) = system_clipboard() else {
        return false;
    };
    let content = make_content(text);
    if clipboard.set_content(Some(&content)).is_err() {
        expire_provider(&content);
        return false;
    }
    watch_clipboard(&clipboard);

    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        cancel_timer(&mut state);
        expire_content(&mut state);
        state.content = Some(content);
        let copied_at = Instant::now();
        state.active = Some(ActiveCopy {
            kind,
            copied_at,
            expires_at: copied_at + clear_after,
        });
        let generation = state.ledger.record_copy();
        state.timer = Some(glib::timeout_add_local_once(clear_after, move || {
            STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                // This source is being dispatched and removes itself: forget its id so it is
                // never removed a second time.
                state.timer = None;
                expire_content(&mut state);
                state.active = None;
                if let Some(clipboard) = system_clipboard()
                    && state.ledger.expire(generation, clipboard.is_local())
                {
                    wipe_owned_clipboard(&clipboard);
                }
            });
            notify();
        }));
    });
    notify();
    true
}

/// Expires a sensitive copy at once (lock, logout, exit). Content copied since by another
/// application is left alone.
pub fn clear_now() {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        cancel_timer(&mut state);
        expire_content(&mut state);
        state.active = None;
        if let Some(clipboard) = system_clipboard()
            && state.ledger.take_pending(clipboard.is_local())
        {
            wipe_owned_clipboard(&clipboard);
        }
    });
    notify();
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use std::time::Instant;

    use super::{
        ActiveCopy, Exposure, Ledger, SNAPSHOT_WINDOW, SensitiveKind, exposure_of, serves_secret,
    };

    #[test]
    fn exposure_reports_the_clipboard_copy_first() {
        let copied_at = Instant::now();
        let expires_at = copied_at + Duration::from_secs(20);
        let copy = ActiveCopy {
            kind: SensitiveKind::Login,
            copied_at,
            expires_at,
        };

        assert_eq!(exposure_of(None, 0), Exposure::Idle);
        assert_eq!(exposure_of(None, 1), Exposure::Decrypting);
        assert_eq!(
            exposure_of(Some(copy), 1),
            Exposure::InClipboard {
                kind: SensitiveKind::Login,
                copied_at,
                expires_at,
            }
        );
    }

    #[test]
    fn the_compositor_snapshot_never_receives_the_secret() {
        // Mutter reads the new clipboard within a few milliseconds and republishes that
        // snapshot after the application exits.
        assert!(!serves_secret(Duration::ZERO, false));
        assert!(!serves_secret(Duration::from_millis(3), false));
        assert!(!serves_secret(
            SNAPSHOT_WINDOW - Duration::from_millis(1),
            false
        ));
    }

    #[test]
    fn a_paste_receives_the_secret_until_it_expires() {
        assert!(serves_secret(SNAPSHOT_WINDOW, false));
        assert!(serves_secret(Duration::from_secs(19), false));
        assert!(!serves_secret(Duration::from_secs(19), true));
    }

    #[test]
    fn the_timer_of_the_latest_copy_clears_it() {
        let mut ledger = Ledger::default();
        let generation = ledger.record_copy();

        assert!(ledger.expire(generation, true));
        assert!(
            !ledger.take_pending(true),
            "a cleared copy is not pending anymore"
        );
    }

    #[test]
    fn an_older_timer_never_clears_a_newer_copy() {
        let mut ledger = Ledger::default();
        let first = ledger.record_copy();
        let second = ledger.record_copy();

        assert!(!ledger.expire(first, true));
        assert!(ledger.expire(second, true));
    }

    #[test]
    fn content_copied_by_another_application_is_never_wiped() {
        let mut ledger = Ledger::default();
        let generation = ledger.record_copy();

        assert!(!ledger.expire(generation, false));
        assert!(
            !ledger.take_pending(true),
            "once expired, a lock must not wipe what the user copied since"
        );
    }

    #[test]
    fn locking_clears_a_pending_copy_once() {
        let mut ledger = Ledger::default();
        let generation = ledger.record_copy();

        assert!(ledger.take_pending(true));
        assert!(!ledger.take_pending(true));
        assert!(
            !ledger.expire(generation, true),
            "the timer of a copy cleared at lock time must do nothing"
        );
    }

    #[test]
    fn locking_leaves_the_clipboard_alone_when_nothing_sensitive_was_copied() {
        let mut ledger = Ledger::default();

        assert!(!ledger.take_pending(true));
    }

    #[test]
    fn locking_does_not_wipe_content_another_application_took_over() {
        let mut ledger = Ledger::default();
        ledger.record_copy();

        assert!(!ledger.take_pending(false));
    }

    /// Every clipboard write of the application must go through this module, or a secret could
    /// be copied without being cleared.
    #[test]
    fn no_other_module_writes_to_the_clipboard() {
        fn scan(dir: &std::path::Path, offenders: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                panic!("cannot read {}", dir.display());
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan(&path, offenders);
                    continue;
                }
                let is_source = path
                    .extension()
                    .is_some_and(|extension| extension == "rs" || extension == "inc");
                if !is_source || path.ends_with("sensitive_clipboard.rs") {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(&path) else {
                    panic!("cannot read {}", path.display());
                };
                for (number, line) in content.lines().enumerate() {
                    let code = line.split("//").next().unwrap_or_default();
                    // Every GTK clipboard is reached through one of these accessors.
                    if code.contains(".clipboard()") || code.contains(".primary_clipboard()") {
                        offenders.push(format!("{}:{}", path.display(), number + 1));
                    }
                }
            }
        }

        let mut offenders = Vec::new();
        scan(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut offenders,
        );
        assert!(
            offenders.is_empty(),
            "use ui::sensitive_clipboard instead of writing to the clipboard directly:\n{}",
            offenders.join("\n")
        );
    }
}

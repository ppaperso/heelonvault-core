use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

fn network_key(result: Result<(), CheckError>) -> (&'static str, &'static str) {
    match result {
        Ok(()) => ("security-url-tls-ok", "url-security-ok"),
        Err(CheckError::Tls) => ("security-url-tls-error", "url-security-error"),
        Err(CheckError::NonPublicAddress) => ("security-url-blocked", "url-security-warning"),
        Err(CheckError::Timeout) => ("security-url-timeout", ""),
        Err(CheckError::Resolution) => ("security-url-dns-error", ""),
        Err(CheckError::Connection) => ("security-url-connection-error", ""),
        _ => ("security-url-check-error", "url-security-warning"),
    }
}

struct State {
    generation: Cell<u64>,
    task: RefCell<Option<tokio::task::AbortHandle>>,
    local: gtk4::Label,
    transport: gtk4::Label,
    button: gtk4::Button,
}

impl State {
    fn cancel(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.button.set_sensitive(true);
        self.transport.set_text(&tr("security-url-network-off"));
        for class in [
            "url-security-ok",
            "url-security-warning",
            "url-security-error",
        ] {
            self.transport.remove_css_class(class);
        }
    }

    fn local_status(&self, raw: &str) {
        let status = assess_url(raw);
        self.local.set_text(&tr(url_status_key(status)));
        for class in [
            "url-security-ok",
            "url-security-warning",
            "url-security-error",
        ] {
            self.local.remove_css_class(class);
        }
        let class = match status {
            UrlStatus::Invalid | UrlStatus::UnsupportedScheme | UrlStatus::Credentials => {
                "url-security-error"
            }
            UrlStatus::Http | UrlStatus::LocalHost | UrlStatus::InternationalDomain => {
                "url-security-warning"
            }
            _ => "",
        };
        if !class.is_empty() {
            self.local.add_css_class(class);
        }
        self.button.set_sensitive(matches!(
            status,
            UrlStatus::HttpsUnverified | UrlStatus::InternationalDomain
        ));
    }
}

/// Each explicit check gets its own consent; changing the value or leaving the editor aborts it.
pub(crate) fn attach(entry: &gtk4::Entry, row: &gtk4::Box, runtime: Handle) {
    let local = gtk4::Label::new(None);
    let transport = gtk4::Label::new(Some(&tr("security-url-network-off")));
    for label in [&local, &transport] {
        label.set_wrap(true);
        label.set_xalign(0.0);
        label.add_css_class("url-security-status");
        row.append(label);
    }
    let button = gtk4::Button::with_label(&tr("security-url-check"));
    button.set_halign(gtk4::Align::Start);
    row.append(&button);
    let reputation = gtk4::Label::new(Some(&tr("security-url-reputation-off")));
    reputation.set_xalign(0.0);
    reputation.set_wrap(true);
    reputation.add_css_class("dim-label");
    row.append(&reputation);
    let state = Rc::new(State {
        generation: Cell::new(0),
        task: RefCell::new(None),
        local,
        transport,
        button,
    });
    state.local_status(entry.text().as_str());
    let changed = Rc::clone(&state);
    entry.connect_changed(move |entry| {
        changed.cancel();
        changed.local_status(entry.text().as_str());
    });
    let unmapped = Rc::clone(&state);
    row.connect_unmap(move |_| unmapped.cancel());
    let weak = Rc::downgrade(&state);
    let entry = entry.downgrade();
    state.button.connect_clicked(move |_| {
        let (Some(state), Some(entry)) = (weak.upgrade(), entry.upgrade()) else {
            return;
        };
        let Ok(parsed) = url::Url::parse(entry.text().as_str()) else {
            return;
        };
        let origin = parsed.origin().ascii_serialization();
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
        let weak = Rc::downgrade(&state);
        let entry = entry.downgrade();
        let runtime = runtime.clone();
        dialog.connect_response(None, move |_, response| {
            let (Some(state), Some(entry)) = (weak.upgrade(), entry.upgrade()) else {
                return;
            };
            if response != "check" || generation != state.generation.get() || !entry.is_mapped() {
                return;
            }
            state.button.set_sensitive(false);
            state.transport.set_text(&tr("security-url-checking"));
            let target = origin.clone();
            let task =
                runtime.spawn(async move { heelonvault_url_check::check_https(&target).await });
            *state.task.borrow_mut() = Some(task.abort_handle());
            let weak = Rc::downgrade(&state);
            let entry = entry.downgrade();
            glib::MainContext::default().spawn_local(async move {
                let result = task.await;
                let (Some(state), Some(entry)) = (weak.upgrade(), entry.upgrade()) else {
                    return;
                };
                if generation != state.generation.get() || !entry.is_mapped() {
                    return;
                }
                state.task.borrow_mut().take();
                state.button.set_sensitive(true);
                let (key, class) = match result {
                    Ok(result) => network_key(result),
                    Err(_) => ("security-url-check-error", ""),
                };
                for old in [
                    "url-security-ok",
                    "url-security-warning",
                    "url-security-error",
                ] {
                    state.transport.remove_css_class(old);
                }
                if !class.is_empty() {
                    state.transport.add_css_class(class);
                }
                state.transport.set_text(&tr_args(
                    "security-url-result",
                    &[
                        ("result", I18nArg::Str(&tr(key))),
                        (
                            "time",
                            I18nArg::Str(&chrono::Local::now().format("%H:%M").to_string()),
                        ),
                    ],
                ));
            });
        });
        dialog.present();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_network_is_not_reported_as_malicious() {
        assert_eq!(network_key(Err(CheckError::Resolution)).1, "");
        assert_eq!(network_key(Err(CheckError::Timeout)).1, "");
        assert_eq!(network_key(Err(CheckError::Tls)).1, "url-security-error");
        assert_eq!(network_key(Ok(())).0, "security-url-tls-ok");
    }
}

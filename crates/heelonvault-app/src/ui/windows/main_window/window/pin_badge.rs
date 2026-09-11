//! Header PIN badge state.
//!
//! The badge mirrors the PIN cache lifetime: it re-colors as the remaining time shrinks
//! and clears itself when the cache expires, so a stale "PIN active" pill is never shown.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;

use heelonvault_core::services::pin_cache_service::PinCache;

use crate::ui::dialogs::pin_unlock_dialog::PIN_HARD_TIMEOUT;

const WARN_SECS: u64 = 2 * 3600;
const CRIT_SECS: u64 = 15 * 60;
/// Refresh cadence of the badge countdown.
const TICK: Duration = Duration::from_secs(60);

fn fmt_remaining(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{}m", minutes.max(1))
    }
}

fn clear_status_classes(label: &gtk4::Label) {
    label.remove_css_class("pin-status-nominal");
    label.remove_css_class("pin-status-warning");
    label.remove_css_class("pin-status-critical");
}

fn apply_visual(label: &gtk4::Label, button: &gtk4::Button, remaining_secs: u64) {
    clear_status_classes(label);
    if remaining_secs > WARN_SECS {
        label.set_text(heelonvault_core::tr!("pin-status-active").as_str());
        label.add_css_class("pin-status-nominal");
    } else if remaining_secs > CRIT_SECS {
        label.set_text(heelonvault_core::tr!("pin-status-active").as_str());
        label.add_css_class("pin-status-warning");
    } else {
        let minutes = (remaining_secs / 60).max(1);
        label.set_text(&format!("PIN · {minutes}m"));
        label.add_css_class("pin-status-critical");
    }

    let remaining = fmt_remaining(remaining_secs);
    button.set_tooltip_text(Some(
        heelonvault_core::i18n::tr_args(
            "pin-tooltip-secure",
            &[(
                "remaining",
                heelonvault_core::i18n::I18nArg::Str(remaining.as_str()),
            )],
        )
        .as_str(),
    ));
}

fn show_inactive(label: &gtk4::Label, button: &gtk4::Button) {
    label.set_text(heelonvault_core::tr!("pin-status-inactive").as_str());
    label.remove_css_class("status-role-user");
    label.add_css_class("status-role-disabled");
    clear_status_classes(label);
    button.set_tooltip_text(None);
}

fn remaining_secs(pin_cache: &Rc<RefCell<Option<PinCache>>>) -> Option<u64> {
    pin_cache
        .borrow()
        .as_ref()
        .map(|cache| cache.remaining(PIN_HARD_TIMEOUT).as_secs())
}

/// Build the callback invoked whenever the PIN cache is created or dropped.
pub fn build_pin_state_callback(
    header_pin_label: gtk4::Label,
    header_pin_button: gtk4::Button,
    pin_cache: Rc<RefCell<Option<PinCache>>>,
) -> Rc<dyn Fn(bool)> {
    let countdown: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    Rc::new(move |active: bool| {
        if let Some(previous) = countdown.borrow_mut().take() {
            previous.remove();
        }

        if !active {
            show_inactive(&header_pin_label, &header_pin_button);
            return;
        }

        header_pin_label.remove_css_class("status-role-disabled");
        header_pin_label.add_css_class("status-role-user");
        apply_visual(
            &header_pin_label,
            &header_pin_button,
            remaining_secs(&pin_cache).unwrap_or_else(|| PIN_HARD_TIMEOUT.as_secs()),
        );

        let label_for_tick = header_pin_label.clone();
        let button_for_tick = header_pin_button.clone();
        let cache_for_tick = Rc::clone(&pin_cache);
        let countdown_for_tick = Rc::clone(&countdown);
        let source_id = glib::timeout_add_local(TICK, move || {
            let remaining = remaining_secs(&cache_for_tick).unwrap_or(0);
            if remaining == 0 {
                show_inactive(&label_for_tick, &button_for_tick);
                *countdown_for_tick.borrow_mut() = None;
                return glib::ControlFlow::Break;
            }
            apply_visual(&label_for_tick, &button_for_tick, remaining);
            glib::ControlFlow::Continue
        });
        *countdown.borrow_mut() = Some(source_id);
    })
}

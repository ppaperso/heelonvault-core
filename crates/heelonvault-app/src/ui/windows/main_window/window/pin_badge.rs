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
        label.set_text(
            heelonvault_core::i18n::tr_args(
                "pin-status-critical",
                &[(
                    "minutes",
                    heelonvault_core::i18n::I18nArg::Num(
                        i64::try_from(minutes).unwrap_or(i64::MAX),
                    ),
                )],
            )
            .as_str(),
        );
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

fn refresh_state(
    label: &gtk4::Label,
    button: &gtk4::Button,
    pin_cache: &Rc<RefCell<Option<PinCache>>>,
    hard_timeout: Duration,
    on_state_changed: &dyn Fn(bool),
) -> Option<u64> {
    let remaining = pin_cache
        .borrow()
        .as_ref()
        .filter(|cache| !cache.is_exhausted())
        .map(|cache| cache.remaining(hard_timeout).as_secs())
        .filter(|seconds| *seconds > 0);
    if let Some(seconds) = remaining {
        label.remove_css_class("status-role-disabled");
        label.add_css_class("status-role-user");
        apply_visual(label, button, seconds);
    } else {
        pin_cache.borrow_mut().take();
        show_inactive(label, button);
    }
    on_state_changed(remaining.is_some());
    remaining
}

/// Build the callback invoked whenever the PIN cache is created or dropped.
pub fn build_pin_state_callback(
    header_pin_label: gtk4::Label,
    header_pin_button: gtk4::Button,
    pin_cache: Rc<RefCell<Option<PinCache>>>,
    on_state_changed: Rc<dyn Fn(bool)>,
) -> Rc<dyn Fn(bool)> {
    let countdown: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    Rc::new(move |active: bool| {
        if let Some(previous) = countdown.borrow_mut().take() {
            previous.remove();
        }

        if !active {
            show_inactive(&header_pin_label, &header_pin_button);
            on_state_changed(false);
            return;
        }

        if refresh_state(
            &header_pin_label,
            &header_pin_button,
            &pin_cache,
            PIN_HARD_TIMEOUT,
            on_state_changed.as_ref(),
        )
        .is_none()
        {
            return;
        }

        let label_for_tick = header_pin_label.clone();
        let button_for_tick = header_pin_button.clone();
        let cache_for_tick = Rc::clone(&pin_cache);
        let countdown_for_tick = Rc::clone(&countdown);
        let on_state_for_tick = Rc::clone(&on_state_changed);
        let source_id = glib::timeout_add_local(TICK, move || {
            if refresh_state(
                &label_for_tick,
                &button_for_tick,
                &cache_for_tick,
                PIN_HARD_TIMEOUT,
                on_state_for_tick.as_ref(),
            )
            .is_none()
            {
                *countdown_for_tick.borrow_mut() = None;
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        *countdown.borrow_mut() = Some(source_id);
    })
}

#[cfg(test)]
pub(in crate::ui::windows::main_window) fn expire_for_test(
    label: &gtk4::Label,
    button: &gtk4::Button,
    pin_cache: &Rc<RefCell<Option<PinCache>>>,
    on_state_changed: &dyn Fn(bool),
) {
    assert!(refresh_state(label, button, pin_cache, Duration::ZERO, on_state_changed).is_none());
}

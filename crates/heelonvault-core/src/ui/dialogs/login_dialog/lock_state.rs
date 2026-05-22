use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;

use crate::i18n::I18nArg;

pub(super) fn start_lock_countdown(
    button: &gtk4::Button,
    spinner: &gtk4::Spinner,
    error_label: &gtk4::Label,
    remaining_secs: i64,
    lock_active: Rc<Cell<bool>>,
    lock_timer: Rc<RefCell<Option<glib::SourceId>>>,
    set_pending_state: impl Fn(&gtk4::Button, &gtk4::Spinner, bool) + Clone + 'static,
    show_feedback: impl Fn(&gtk4::Label, &str) + Clone + 'static,
) {
    if let Some(source_id) = lock_timer.borrow_mut().take() {
        source_id.remove();
    }

    let mut current_secs = remaining_secs.max(0);
    if current_secs == 0 {
        lock_active.set(false);
        set_pending_state(button, spinner, false);
        show_feedback(error_label, crate::tr!("login-error-retry-now").as_str());
        return;
    }

    lock_active.set(true);
    spinner.set_visible(false);
    spinner.set_spinning(false);
    button.set_sensitive(false);

    show_feedback(
        error_label,
        crate::i18n::tr_args(
            "login-error-account-locked",
            &[("seconds", I18nArg::Num(current_secs))],
        )
        .as_str(),
    );

    let button_for_tick = button.clone();
    let spinner_for_tick = spinner.clone();
    let error_for_tick = error_label.clone();
    let lock_active_for_tick = Rc::clone(&lock_active);
    let lock_timer_for_tick = Rc::clone(&lock_timer);
    let show_feedback_for_tick = show_feedback.clone();
    let set_pending_for_tick = set_pending_state.clone();
    let source_id = glib::timeout_add_seconds_local(1, move || {
        current_secs = current_secs.saturating_sub(1);
        if current_secs == 0 {
            lock_active_for_tick.set(false);
            set_pending_for_tick(&button_for_tick, &spinner_for_tick, false);
            show_feedback_for_tick(
                &error_for_tick,
                crate::tr!("login-error-retry-now").as_str(),
            );
            let _ = lock_timer_for_tick.borrow_mut().take();
            glib::ControlFlow::Break
        } else {
            show_feedback_for_tick(
                &error_for_tick,
                crate::i18n::tr_args(
                    "login-error-account-locked",
                    &[("seconds", I18nArg::Num(current_secs))],
                )
                .as_str(),
            );
            glib::ControlFlow::Continue
        }
    });

    *lock_timer.borrow_mut() = Some(source_id);
}

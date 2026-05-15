use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::WidgetExt;
use libadwaita as adw;
use tracing::info;
use zeroize::Zeroize;

pub(super) fn activate_auto_lock(
    window: &adw::ApplicationWindow,
    auto_lock_timeout_secs: &Rc<Cell<u64>>,
    auto_lock_source: &Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: &Rc<Cell<bool>>,
    on_auto_lock: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    session_master_key: &Rc<RefCell<Vec<u8>>>,
) {
    if auto_lock_timeout_secs.get() == 0 {
        auto_lock_armed.set(false);
        return;
    }

    auto_lock_armed.set(true);
    reset_auto_lock_timer(
        window,
        auto_lock_source,
        auto_lock_armed,
        auto_lock_timeout_secs.get(),
        on_auto_lock,
        session_master_key,
    );
}

pub(super) fn set_auto_lock_timeout(
    window: &adw::ApplicationWindow,
    auto_lock_timeout_secs: &Rc<Cell<u64>>,
    auto_lock_source: &Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: &Rc<Cell<bool>>,
    on_auto_lock: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    session_master_key: &Rc<RefCell<Vec<u8>>>,
    mins: u64,
) {
    let mins = match mins {
        0 | 1 | 5 | 10 | 15 | 30 => mins,
        _ => 5,
    };

    auto_lock_timeout_secs.set(mins.saturating_mul(60));
    if mins == 0 {
        deactivate_auto_lock(auto_lock_source, auto_lock_armed);
        return;
    }

    if auto_lock_armed.get() {
        reset_auto_lock_timer(
            window,
            auto_lock_source,
            auto_lock_armed,
            auto_lock_timeout_secs.get(),
            on_auto_lock,
            session_master_key,
        );
    }
}

pub(super) fn deactivate_auto_lock(
    auto_lock_source: &Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: &Rc<Cell<bool>>,
) {
    auto_lock_armed.set(false);
    if let Some(source_id) = auto_lock_source.borrow_mut().take() {
        source_id.remove();
    }
}

pub(super) fn reset_auto_lock_timer(
    window: &adw::ApplicationWindow,
    auto_lock_source: &Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: &Rc<Cell<bool>>,
    timeout_secs: u64,
    on_auto_lock: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    session_master_key: &Rc<RefCell<Vec<u8>>>,
) {
    if !auto_lock_armed.get() || !window.is_visible() || timeout_secs == 0 {
        return;
    }

    if let Some(source_id) = auto_lock_source.borrow_mut().take() {
        source_id.remove();
    }

    let auto_lock_source_for_timeout = Rc::clone(auto_lock_source);
    let auto_lock_armed_for_timeout = Rc::clone(auto_lock_armed);
    let on_auto_lock_for_timeout = Rc::clone(on_auto_lock);
    let session_master_for_timeout = Rc::clone(session_master_key);
    let source_id = glib::timeout_add_local_once(Duration::from_secs(timeout_secs), move || {
        if !auto_lock_armed_for_timeout.get() {
            return;
        }
        if let Some(active_source) = auto_lock_source_for_timeout.borrow_mut().take() {
            active_source.remove();
        }
        auto_lock_armed_for_timeout.set(false);
        {
            let mut key = session_master_for_timeout.borrow_mut();
            key.zeroize();
            key.clear();
        }
        info!("Auto-lock triggered due to inactivity");
        if let Some(callback) = on_auto_lock_for_timeout.borrow().as_ref() {
            callback();
        }
    });

    *auto_lock_source.borrow_mut() = Some(source_id);
}

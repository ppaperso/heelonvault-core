//! Types module - Data structures for main window widgets and state
//!
//! This module contains all the data structures needed to represent
//! the main window's UI components and shared state.
//!
//! For now, these are kept minimal. They will be expanded as the
//! refactor progresses and code is migrated from the .inc files.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::glib;
use libadwaita as adw;

/// Main window widgets structure - contains all UI components
/// This will be populated as the refactor progresses
#[derive(Clone)]
#[allow(dead_code)]
pub struct MainWindowWidgets {
    pub window: adw::ApplicationWindow,
}

/// Main window state structure
/// This will be populated as the refactor progresses
#[allow(dead_code)]
pub struct MainWindowState {
    pub auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
    pub auto_lock_armed: Rc<Cell<bool>>,
    pub auto_lock_timeout_secs: Rc<Cell<u64>>,
}

#[allow(dead_code)]
impl MainWindowState {
    pub fn new() -> Self {
        Self {
            auto_lock_source: Rc::new(RefCell::new(None)),
            auto_lock_armed: Rc::new(Cell::new(false)),
            auto_lock_timeout_secs: Rc::new(Cell::new(0)),
        }
    }
}

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, InputPurpose, Orientation};
use libadwaita as adw;
use tracing::warn;

use crate::services::pin_cache_service::PinUnlockError;
use crate::ui::windows::main_window::MainWindow;

/// Hard-timeout: if the session is older than this, the PIN cache is no longer
/// accepted and the user must enter the full master password.
pub const PIN_HARD_TIMEOUT: Duration = Duration::from_secs(12 * 3600); // 12 h

/// A modal PIN entry dialog shown when auto-lock fires and a PIN cache is set.
pub struct PinUnlockDialog {
    window: gtk4::Window,
}

impl PinUnlockDialog {
    /// Build and return the dialog.
    ///
    /// * `main` — the active `MainWindow` (for `try_pin_unlock` + session key restore)
    /// * `on_unlocked` — called with the master-key bytes once the PIN is verified
    /// * `on_use_master_password` — called when the user chooses "Use master password"
    pub fn new(
        parent: &adw::ApplicationWindow,
        main: Rc<MainWindow>,
        on_unlocked: impl Fn(Vec<u8>) + 'static,
        on_use_master_password: impl Fn() + 'static,
    ) -> Self {
        include!("parts/new_body.inc")
    }

    pub fn present(&self) {
        self.window.present();
    }
}

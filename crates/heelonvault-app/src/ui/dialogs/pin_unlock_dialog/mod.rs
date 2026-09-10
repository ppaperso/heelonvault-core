use std::time::Duration;

use gtk4::prelude::GtkWindowExt;
use libadwaita as adw;

mod core;
mod events;
mod feedback;
mod types;
mod views;

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
    /// * `on_unlocked` — called with `Some(master_key)` once the PIN is verified, or
    ///   `None` when the cache is exhausted (caller should fall back to full login)
    /// * `on_use_master_password` — called when the user chooses "Use master password"
    pub fn new(
        parent: &adw::ApplicationWindow,
        main: std::rc::Rc<crate::ui::windows::main_window::MainWindow>,
        on_unlocked: impl Fn(Option<zeroize::Zeroizing<Vec<u8>>>) + 'static,
        on_use_master_password: impl Fn() + 'static,
    ) -> Self {
        core::build_dialog(parent, main, on_unlocked, on_use_master_password)
    }

    pub fn present(&self) {
        self.window.present();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::PinUnlockDialogWidgets;

    #[test]
    fn test_pin_unlock_dialog_structure_exists() {
        let _dialog_type = std::any::TypeId::of::<PinUnlockDialog>();
    }

    #[test]
    fn test_pin_unlock_dialog_widgets_structure_exists() {
        let _widgets_type = std::any::TypeId::of::<PinUnlockDialogWidgets>();
    }

    #[test]
    #[allow(unused_imports)]
    fn test_feedback_functions_are_accessible() {
        use super::feedback::{hide_feedback, show_feedback};
    }

    #[test]
    #[allow(unused_imports)]
    fn test_events_functions_are_accessible() {
        use super::events::{setup_fallback_handler, setup_feedback_reset, setup_quit_handler, setup_unlock_handler};
    }

    #[test]
    #[allow(unused_imports)]
    fn test_views_function_is_accessible() {
        use super::views::build_pin_unlock_view;
    }

    #[test]
    #[allow(unused_imports)]
    fn test_core_function_is_accessible() {
        use super::core::build_dialog;
    }
}


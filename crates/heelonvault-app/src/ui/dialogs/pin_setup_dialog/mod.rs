use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, InputPurpose, Orientation};
use libadwaita as adw;
use secrecy::SecretBox;
use uuid::Uuid;

use heelonvault_core::services::pin_cache_service::{
    PIN_MAX_LEN, PIN_MIN_LEN, PinCache, validate_pin,
};

/// Dialog for setting or disabling the quick-unlock PIN.
pub struct PinSetupDialog {
    window: gtk4::Window,
}

impl PinSetupDialog {
    /// Build the dialog.
    ///
    /// * `master_key` — the current session master key (needed to wrap with the new PIN)
    /// * `user_id`    — bound to the resulting `PinCache`
    /// * `on_cache_created` — called with the new `PinCache` on success
    /// * `on_pin_disabled`  — called when the user explicitly disables the PIN
    pub fn new(
        parent: &adw::ApplicationWindow,
        master_key: SecretBox<Vec<u8>>,
        user_id: Uuid,
        on_cache_created: impl Fn(PinCache) + 'static,
        on_pin_disabled: impl Fn() + 'static,
        pin_currently_active: bool,
    ) -> Self {
        include!("parts/new_body.inc")
    }

    pub fn present(&self) {
        self.window.present();
    }
}

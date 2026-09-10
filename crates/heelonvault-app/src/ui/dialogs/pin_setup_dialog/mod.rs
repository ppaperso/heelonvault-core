use gtk4::prelude::GtkWindowExt;
use libadwaita as adw;
use secrecy::SecretBox;
use uuid::Uuid;

use heelonvault_core::services::pin_cache_service::PinCache;

mod core;
mod events;
mod feedback;
mod types;
mod views;

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
        core::build_dialog(
            parent,
            master_key,
            user_id,
            on_cache_created,
            on_pin_disabled,
            pin_currently_active,
        )
    }

    pub fn present(&self) {
        self.window.present();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::PinSetupDialogWidgets;

    #[test]
    fn test_pin_setup_dialog_structure_exists() {
        // Vérifie que la structure PinSetupDialog existe
        let _dialog_type = std::any::TypeId::of::<PinSetupDialog>();
    }

    #[test]
    fn test_pin_setup_dialog_widgets_structure_exists() {
        // Vérifie que la structure PinSetupDialogWidgets existe
        let _widgets_type = std::any::TypeId::of::<PinSetupDialogWidgets>();
    }

    #[test]
    #[allow(unused_imports)]
    fn test_feedback_functions_are_accessible() {
        // Vérifie que les fonctions de feedback sont bien exportées
        use super::feedback::{hide_feedback, show_feedback};
        // Si ce code compile, les fonctions existent
    }

    #[test]
    #[allow(unused_imports)]
    fn test_events_functions_are_accessible() {
        // Vérifie que les fonctions d'events sont bien exportées
        use super::events::{setup_disable_handler, setup_feedback_reset, setup_save_handler};
        // Si ce code compile, les fonctions existent
    }

    #[test]
    #[allow(unused_imports)]
    fn test_views_function_is_accessible() {
        // Vérifie que la fonction de construction de l'UI existe
        use super::views::build_pin_setup_view;
        // Si ce code compile, la fonction existe
    }

    #[test]
    #[allow(unused_imports)]
    fn test_core_function_is_accessible() {
        // Vérifie que la fonction core existe
        use super::core::build_dialog;
        // Si ce code compile, la fonction existe
    }
}

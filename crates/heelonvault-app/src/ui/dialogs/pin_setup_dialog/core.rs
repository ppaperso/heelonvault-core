use secrecy::SecretBox;
use std::rc::Rc;
use uuid::Uuid;

use libadwaita as adw;

use super::events;
use super::views;
use heelonvault_core::services::pin_cache_service::PinCache;

/// Construit et configure la dialogue complete de configuration du PIN.
///
/// Cette fonction orchestre la creation de l'UI, la connexion des evenements,
/// et l'assemblage final de la dialogue.
///
/// # Arguments
/// * `parent` - La fenetre parente
/// * `master_key` - La clef maitre pour créer le PinCache
/// * `user_id` - L'ID utilisateur pour le PinCache
/// * `on_cache_created` - Callback appele avec le nouveau PinCache
/// * `on_pin_disabled` - Callback appele lorsque le PIN est desactive
/// * `pin_currently_active` - Si vrai, le bouton de desactivation est affiche
///
/// # Returns
/// Une nouvelle instance de `PinSetupDialog`
pub fn build_dialog(
    parent: &adw::ApplicationWindow,
    master_key: SecretBox<Vec<u8>>,
    user_id: Uuid,
    on_cache_created: impl Fn(PinCache) + 'static,
    on_pin_disabled: impl Fn() + 'static,
    pin_currently_active: bool,
) -> super::PinSetupDialog {
    // 1. Construire l'UI
    let widgets = views::build_pin_setup_view(parent, pin_currently_active);

    // 2. Configurer les handlers de feedback reset
    events::setup_feedback_reset(&widgets);

    // 3. Configurer le handler save
    let master_key_rc = Rc::new(master_key);
    let on_cache_created_rc: Rc<dyn Fn(PinCache)> = Rc::new(on_cache_created);
    let window_for_save = widgets.window.clone();

    events::setup_save_handler(
        &widgets,
        window_for_save,
        master_key_rc,
        user_id,
        on_cache_created_rc,
    );

    // 4. Configurer le handler disable (si applicable)
    let on_pin_disabled_rc: Rc<dyn Fn()> = Rc::new(on_pin_disabled);
    let window_for_disable = widgets.window.clone();

    events::setup_disable_handler(&widgets, window_for_disable, on_pin_disabled_rc);

    // 5. Retourner la structure
    super::PinSetupDialog {
        window: widgets.window,
    }
}

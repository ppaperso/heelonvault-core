use std::rc::Rc;
use gtk4::prelude::*;
use secrecy::SecretBox;
use uuid::Uuid;

use heelonvault_core::services::pin_cache_service::{PinCache, validate_pin};
use super::feedback;
use super::types::PinSetupDialogWidgets;

/// Connecte le handler du bouton save.
///
/// Ce handler valide le PIN et la confirmation, puis cree un PinCache.
///
/// # Arguments
/// * `widgets` - Les widgets de la dialogue
/// * `window` - La fenetre a fermer sur succes
/// * `master_key` - La clef maitre pour créer le PinCache
/// * `user_id` - L'ID utilisateur pour le PinCache
/// * `on_cache_created` - Callback appele avec le nouveau PinCache
pub fn setup_save_handler(
    widgets: &PinSetupDialogWidgets,
    window: gtk4::Window,
    master_key: Rc<SecretBox<Vec<u8>>>,
    user_id: Uuid,
    on_cache_created: Rc<dyn Fn(PinCache)>,
) {
    let win_for_save = window;
    let pin_for_save = widgets.pin_entry.clone();
    let confirm_for_save = widgets.confirm_entry.clone();
    let feedback_for_save = widgets.feedback_label.clone();

    widgets.save_button.connect_clicked(move |_| {
        let pin = pin_for_save.text().to_string();
        let confirm = confirm_for_save.text().to_string();

        if pin != confirm {
            feedback::show_feedback(
                &feedback_for_save,
                heelonvault_core::tr!("pin-setup-error-mismatch").as_str(),
                true,
            );
            return;
        }

        if let Err(e) = validate_pin(&pin) {
            feedback::show_feedback(&feedback_for_save, &e.to_string(), true);
            return;
        }

        match PinCache::wrap(master_key.as_ref(), &pin, user_id) {
            Ok(cache) => {
                win_for_save.close();
                on_cache_created(cache);
            }
            Err(e) => {
                feedback::show_feedback(
                    &feedback_for_save,
                    &format!("{}: {e}", heelonvault_core::tr!("pin-setup-error-internal").as_str()),
                    true,
                );
            }
        }
    });
}

/// Connecte le handler du bouton disable (si present).
///
/// # Arguments
/// * `widgets` - Les widgets de la dialogue
/// * `window` - La fenetre a fermer
/// * `on_pin_disabled` - Callback appele lorsque le PIN est desactive
pub fn setup_disable_handler(
    widgets: &PinSetupDialogWidgets,
    window: gtk4::Window,
    on_pin_disabled: Rc<dyn Fn()>,
) {
    if let Some(disable_button) = &widgets.disable_button {
        let win_for_disable = window;
        let on_pin_disabled_for_btn = Rc::clone(&on_pin_disabled);
        disable_button.connect_clicked(move |_| {
            win_for_disable.close();
            on_pin_disabled_for_btn();
        });
    }
}

/// Connecte les handlers pour cacher le feedback lors des changements de texte.
///
/// # Arguments
/// * `widgets` - Les widgets de la dialogue
pub fn setup_feedback_reset(widgets: &PinSetupDialogWidgets) {
    let feedback_for_pin = widgets.feedback_label.clone();
    let feedback_for_confirm = widgets.feedback_label.clone();

    widgets.pin_entry.connect_changed(move |_| {
        feedback::hide_feedback(&feedback_for_pin);
    });

    widgets.confirm_entry.connect_changed(move |_| {
        feedback::hide_feedback(&feedback_for_confirm);
    });
}

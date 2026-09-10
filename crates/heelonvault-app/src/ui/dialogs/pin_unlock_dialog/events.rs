use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use zeroize::Zeroizing;

use crate::ui::windows::main_window::MainWindow;
use heelonvault_core::services::pin_cache_service::PinUnlockError;

use super::feedback;
use super::types::PinUnlockDialogWidgets;

/// Connecte le handler du bouton unlock et de l'entry PIN.
///
/// Ce handler valide le PIN et appele le callback on_unlocked en cas de succes.
///
/// # Arguments
/// * `widgets` - Les widgets de la dialogue
/// * `window` - La fenetre a fermer
/// * `main` - Reference vers la fenetre principale
/// * `on_unlocked` - Callback appele avec Some(master_key) ou None
/// * `locked` - Etat partage pour bloquer les tentatives repetes
pub fn setup_unlock_handler(
    widgets: &PinUnlockDialogWidgets,
    window: gtk4::Window,
    main: Rc<MainWindow>,
    on_unlocked: Rc<dyn Fn(Option<Zeroizing<Vec<u8>>>)>,
    locked: Rc<Cell<bool>>,
) {
    let win_for_unlock = window;
    let main_for_unlock = Rc::clone(&main);
    let pin_entry_for_unlock = widgets.pin_entry.clone();
    let feedback_for_unlock = widgets.feedback_label.clone();
    let locked_for_unlock = Rc::clone(&locked);
    let on_unlocked_for_btn = Rc::clone(&on_unlocked);

    let do_unlock = move || {
        if locked_for_unlock.get() {
            return;
        }
        let pin = pin_entry_for_unlock.text().to_string();
        if pin.is_empty() {
            feedback::show_feedback(
                &feedback_for_unlock,
                heelonvault_core::tr!("pin-error-empty").as_str(),
                true,
            );
            return;
        }

        locked_for_unlock.set(true);
        feedback::hide_feedback(&feedback_for_unlock);

        match main_for_unlock.try_pin_unlock(&pin) {
            Ok(master_key) => {
                win_for_unlock.close();
                on_unlocked_for_btn(Some(master_key));
            }
            Err(PinUnlockError::WrongPin { remaining }) => {
                locked_for_unlock.set(false);
                pin_entry_for_unlock.set_text("");
                let msg = if remaining == 1 {
                    heelonvault_core::i18n::tr("pin-error-wrong-last")
                } else {
                    heelonvault_core::i18n::tr_args("pin-error-wrong", &[("remaining", heelonvault_core::i18n::I18nArg::Num(remaining as i64))])
                };
                feedback::show_feedback(&feedback_for_unlock, &msg, true);

                // Small enforced delay between attempts (1 s) to slow manual brute-force.
                let locked_delay = Rc::clone(&locked_for_unlock);
                glib::timeout_add_local_once(Duration::from_millis(1000), move || {
                    locked_delay.set(false);
                });
            }
            Err(PinUnlockError::Exhausted) => {
                // Cache already wiped by try_pin_unlock.  Close dialog and fall
                // back to full master-password login.
                tracing::warn!("PIN cache exhausted after max failed attempts — falling back to full login");
                win_for_unlock.close();
                on_unlocked_for_btn(None); // None signals cache exhaustion to caller
            }
        }
    };

    let do_unlock_rc = Rc::new(do_unlock);
    let do_unlock_for_btn = Rc::clone(&do_unlock_rc);
    let do_unlock_for_entry = Rc::clone(&do_unlock_rc);

    widgets.unlock_button.connect_clicked(move |_| do_unlock_for_btn());
    widgets.pin_entry.connect_activate(move |_| do_unlock_for_entry());
}

/// Connecte le handler pour cacher le feedback lors des changements de texte.
///
/// # Arguments
/// * `widgets` - Les widgets de la dialogue
pub fn setup_feedback_reset(widgets: &PinUnlockDialogWidgets) {
    let feedback_for_change = widgets.feedback_label.clone();
    widgets.pin_entry.connect_changed(move |_| feedback::hide_feedback(&feedback_for_change));
}

/// Connecte le handler du bouton fallback (utiliser le mot de passe maitre).
///
/// # Arguments
/// * `widgets` - Les widgets de la dialogue
/// * `window` - La fenetre a fermer
/// * `main` - Reference vers la fenetre principale
/// * `on_use_master_password` - Callback appele lorsque l'utilisateur choisit le mot de passe maitre
pub fn setup_fallback_handler(
    widgets: &PinUnlockDialogWidgets,
    window: gtk4::Window,
    main: Rc<MainWindow>,
    on_use_master_password: Rc<dyn Fn()>,
) {
    let main_for_fallback = Rc::clone(&main);
    let win_for_fallback = window;
    let on_use_master_password_rc = Rc::clone(&on_use_master_password);
    
    widgets.fallback_button.connect_clicked(move |_| {
        main_for_fallback.clear_pin_cache();
        win_for_fallback.close();
        on_use_master_password_rc();
    });
}

/// Connecte le handler du bouton quit (quitter l'application).
///
/// # Arguments
/// * `widgets` - Les widgets de la dialogue
/// * `window` - La fenetre a fermer
/// * `main` - Reference vers la fenetre principale
/// * `parent` - La fenetre parente
pub fn setup_quit_handler(
    widgets: &PinUnlockDialogWidgets,
    window: gtk4::Window,
    main: Rc<MainWindow>,
    parent: &adw::ApplicationWindow,
) {
    let main_for_quit = Rc::clone(&main);
    let win_for_quit = window;
    let parent_for_quit = parent.clone();
    
    widgets.quit_button.connect_clicked(move |_| {
        let app = parent_for_quit.application();
        main_for_quit.clear_sensitive_session();
        win_for_quit.close();
        if let Some(app) = app {
            app.quit();
        }
    });
}

use std::cell::Cell;
use std::rc::Rc;

use libadwaita as adw;
use zeroize::Zeroizing;

use crate::ui::windows::main_window::MainWindow;

use super::events;
use super::views;

/// Construit et configure la dialogue complete de debitage par PIN.
///
/// Cette fonction orchestrer la creation de l'UI, la connexion des evenements,
/// et l'assemblage final de la dialogue.
///
/// # Arguments
/// * `parent` - La fenetre parente
/// * `main` - Reference vers la fenetre principale
/// * `on_unlocked` - Callback appele avec Some(master_key) ou None en cas d'echec
/// * `on_use_master_password` - Callback appele lorsque l'utilisateur choisit le mot de passe maitre
///
/// # Returns
/// Une nouvelle instance de `PinUnlockDialog`
pub fn build_dialog(
    parent: &adw::ApplicationWindow,
    main: Rc<MainWindow>,
    on_unlocked: impl Fn(Option<Zeroizing<Vec<u8>>>) + 'static,
    on_use_master_password: impl Fn() + 'static,
) -> super::PinUnlockDialog {
    // 1. Construire l'UI
    let widgets = views::build_pin_unlock_view(parent);

    // 2. Creer l'etat partage pour bloquer les tentatives repetes
    let locked = Rc::new(Cell::new(false));

    // 3. Configurer les handlers de feedback reset
    events::setup_feedback_reset(&widgets);

    // 4. Configurer le handler unlock
    let on_unlocked_rc: Rc<dyn Fn(Option<Zeroizing<Vec<u8>>>)> = Rc::new(on_unlocked);
    let window_for_unlock = widgets.window.clone();
    let main_for_unlock = Rc::clone(&main);
    
    events::setup_unlock_handler(
        &widgets,
        window_for_unlock,
        main_for_unlock,
        on_unlocked_rc,
        Rc::clone(&locked),
    );

    // 5. Configurer le handler fallback
    let on_use_master_password_rc: Rc<dyn Fn()> = Rc::new(on_use_master_password);
    let window_for_fallback = widgets.window.clone();
    let main_for_fallback = Rc::clone(&main);
    
    events::setup_fallback_handler(
        &widgets,
        window_for_fallback,
        main_for_fallback,
        on_use_master_password_rc,
    );

    // 6. Configurer le handler quit
    let window_for_quit = widgets.window.clone();
    let main_for_quit = Rc::clone(&main);
    
    events::setup_quit_handler(
        &widgets,
        window_for_quit,
        main_for_quit,
        parent,
    );

    // 7. Retourner la structure
    super::PinUnlockDialog {
        window: widgets.window,
    }
}

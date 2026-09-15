use super::views::LoginDialogWidgets;
use gtk4::prelude::*;

/// Configure les écouteurs de signaux pour les widgets de connexion.
/// Note: Le bouton submit n'est PAS connecté ici pour éviter les boucles infinies.
/// username_entry, password_entry et totp_entry sont connectés pour répondre à la touche Entrée.
pub fn setup_events<F>(widgets: &LoginDialogWidgets, on_submit: F)
where
    F: Fn() + 'static + Clone,
{
    // 1. Appuyer sur "Entrée" dans le champ du nom d'utilisateur
    let user_cb = on_submit.clone();
    widgets.username_entry.connect_activate(move |_| {
        user_cb();
    });

    // 2. Appuyer sur "Entrée" dans le champ du mot de passe
    let pass_cb = on_submit.clone();
    widgets.password_entry.connect_activate(move |_| {
        pass_cb();
    });

    // 3. Appuyer sur "Entrée" dans le champ TOTP (étape 2, une fois affiché)
    let totp_cb = on_submit;
    widgets.totp_entry.connect_activate(move |_| {
        totp_cb();
    });
}

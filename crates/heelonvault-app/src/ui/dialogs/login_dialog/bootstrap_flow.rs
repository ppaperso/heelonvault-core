use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;

use heelonvault_core::errors::AppError;
use heelonvault_core::i18n::I18nArg;
use heelonvault_core::services::admin_service::BootstrapResult;

use super::views::LoginDialogWidgets;
use super::{AuthenticatedSession, feedback};

type BootstrapCallback =
    Arc<dyn Fn(String, Vec<u8>) -> Result<BootstrapResult, AppError> + Send + Sync>;

/// Two distinct word positions in `0..24`, drawn from the OS RNG so they leak nothing
/// about the phrase itself. Falls back to a fixed pair if the RNG is unavailable.
fn pick_verification_indices() -> (usize, usize) {
    let mut bytes = [0_u8; 2];
    if getrandom::fill(&mut bytes).is_err() {
        return (0, 12);
    }
    let index_a = usize::from(bytes[0]) % 24;
    let index_b = (index_a + 1 + usize::from(bytes[1]) % 23) % 24;
    (index_a.min(index_b), index_a.max(index_b))
}

pub(super) fn handle_init_identity_step(
    init_username: &gtk4::Entry,
    error_label: &gtk4::Label,
    gen_key_fn: Option<&Arc<dyn Fn() -> Result<String, AppError> + Send + Sync>>,
    word_labels: &[gtk4::Label],
    init_verify_indices: Rc<Cell<(usize, usize)>>,
    init_verify_hint: &gtk4::Label,
    init_verify_a_label: &gtk4::Label,
    init_verify_b_label: &gtk4::Label,
    init_oath_words: Rc<RefCell<Vec<String>>>,
    _check_init_identity_gate: &Rc<impl Fn()>,
    step_stack: &gtk4::Stack,
    submit_button: &gtk4::Button,
) {
    let username = init_username.text().trim().to_string();
    if username.is_empty() {
        feedback::show_feedback(
            error_label,
            heelonvault_core::tr!("init-error-username-empty").as_str(),
        );
        return;
    }

    if let Some(gen_key_fn) = gen_key_fn {
        match gen_key_fn() {
            Ok(phrase) => {
                let words: Vec<String> = phrase
                    .split_whitespace()
                    .map(|word| word.to_lowercase())
                    .collect();
                if words.len() == 24 {
                    for (index, label) in word_labels.iter().enumerate() {
                        label.set_text(&words[index]);
                    }
                    let (index_a, index_b) = pick_verification_indices();
                    init_verify_indices.set((index_a, index_b));

                    let hint = heelonvault_core::i18n::tr_args(
                        "init-verify-hint",
                        &[
                            ("a", I18nArg::Num((index_a + 1) as i64)),
                            ("b", I18nArg::Num((index_b + 1) as i64)),
                        ],
                    );
                    init_verify_hint.set_text(hint.as_str());

                    let label_a = heelonvault_core::i18n::tr_args(
                        "init-verify-label-a",
                        &[("index", I18nArg::Num((index_a + 1) as i64))],
                    );
                    init_verify_a_label.set_text(label_a.as_str());

                    let label_b = heelonvault_core::i18n::tr_args(
                        "init-verify-label-b",
                        &[("index", I18nArg::Num((index_b + 1) as i64))],
                    );
                    init_verify_b_label.set_text(label_b.as_str());

                    *init_oath_words.borrow_mut() = words;
                    // Désactiver le bouton submit car les entrées de vérification sont vides
                    // La gate oath sera appelée quand les entrées changeront
                    submit_button.set_sensitive(false);
                    step_stack.set_visible_child_name("init-oath");
                } else {
                    feedback::show_feedback(
                        error_label,
                        heelonvault_core::tr!("login-error-internal").as_str(),
                    );
                }
            }
            Err(_) => {
                feedback::show_feedback(
                    error_label,
                    heelonvault_core::tr!("login-error-internal").as_str(),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_init_oath_step(
    init_clipboard_dirty: Rc<Cell<bool>>,
    init_clipboard_timer: Rc<RefCell<Option<glib::SourceId>>>,
    init_username: &gtk4::Entry,
    init_password: &gtk4::PasswordEntry,
    step_stack: &gtk4::Stack,
    init_pending_spinner: &gtk4::Spinner,
    do_bootstrap_fn: Option<BootstrapCallback>,
    dialog: &gtk4::Window,
    error_label: &gtk4::Label,
    submit_button: &gtk4::Button,
    submit_spinner: &gtk4::Spinner,
    authenticated: Rc<Cell<bool>>,
    on_authenticated: Rc<dyn Fn(AuthenticatedSession)>,
) {
    if init_clipboard_dirty.get() {
        if let Some(display) = gtk4::gdk::Display::default() {
            display.clipboard().set_text("");
        }
        if let Some(id) = init_clipboard_timer.borrow_mut().take() {
            id.remove();
        }
        init_clipboard_dirty.set(false);
    }

    let username = init_username.text().trim().to_string();
    let password_bytes = init_password.text().as_bytes().to_vec();
    step_stack.set_visible_child_name("init-pending");
    init_pending_spinner.start();

    let (result_sender, result_receiver) =
        tokio::sync::oneshot::channel::<Result<BootstrapResult, AppError>>();

    std::thread::spawn(move || {
        let result = if let Some(callback) = do_bootstrap_fn {
            callback(username, password_bytes)
        } else {
            Err(AppError::Conflict(
                "bootstrap function unavailable".to_string(),
            ))
        };
        let _ = result_sender.send(result);
    });

    let dialog_for_result = dialog.clone();
    let error_for_result = error_label.clone();
    let step_for_result = step_stack.clone();
    let spinner_result = init_pending_spinner.clone();
    let button_for_result = submit_button.clone();
    let spinner_for_result = submit_spinner.clone();
    let authenticated_for_result = Rc::clone(&authenticated);
    let on_authenticated_for_result = Rc::clone(&on_authenticated);

    glib::MainContext::default().spawn_local(async move {
        match result_receiver.await {
            Ok(Ok(bootstrap_result)) => {
                spinner_result.stop();
                authenticated_for_result.set(true);
                let identity_label = bootstrap_result.username.clone();
                on_authenticated_for_result(AuthenticatedSession {
                    user_id: bootstrap_result.user_id,
                    username: bootstrap_result.username,
                    identity_label,
                    master_key: bootstrap_result.master_key,
                });
                dialog_for_result.close();
            }
            Ok(Err(error)) => {
                spinner_result.stop();
                step_for_result.set_visible_child_name("init-identity");
                feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                let message = match &error {
                    AppError::Conflict(_) => {
                        heelonvault_core::tr!("init-error-already-initialized")
                    }
                    _ => heelonvault_core::tr!("login-error-unavailable"),
                };
                feedback::show_feedback(&error_for_result, message.as_str());
            }
            Err(_) => {
                spinner_result.stop();
                step_for_result.set_visible_child_name("init-identity");
                feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                feedback::show_feedback(
                    &error_for_result,
                    heelonvault_core::tr!("login-error-interrupted").as_str(),
                );
            }
        }
    });
}

/// Configure le handler du bouton submit pour le mode bootstrap.
/// Ce handler gère les transitions entre les étapes :
/// - init-identity -> init-oath (génération de la clé de récupération)
/// - init-oath -> init-pending (vérification des mots et bootstrap)
/// 
/// # Arguments
/// * `widgets` - Tous les widgets de la dialogue
/// * `gen_key_fn` - Fonction pour générer la clé de récupération (optionnelle)
/// * `do_bootstrap_fn` - Fonction pour exécuter le bootstrap
/// * `window` - Fenêtre de la dialogue
/// * `authenticated` - Cellule indiquant si l'authentification a réussi
/// * `on_authenticated` - Callback appelé après authentification réussie
pub(super) fn setup_bootstrap_submit_handler(
    widgets: &LoginDialogWidgets,
    gen_key_fn: Option<Arc<dyn Fn() -> Result<String, AppError> + Send + Sync>>,
    do_bootstrap_fn: Option<BootstrapCallback>,
    window: &gtk4::Window,
    authenticated: Rc<Cell<bool>>,
    on_authenticated: Rc<dyn Fn(AuthenticatedSession)>,
) {
    // État partagé pour les mots de la phrase de récupération
    let init_oath_words: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    
    // Indices des mots à vérifier
    let init_verify_indices: Rc<Cell<(usize, usize)>> = Rc::new(Cell::new((0, 1)));
    
    // État pour suivre si le presse-papier contient la phrase
    let init_clipboard_dirty: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let init_clipboard_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    
    // Cloner les widgets et états pour le handler
    let step_stack = widgets.step_stack.clone();
    let init_username_entry = widgets.init_username_entry.clone();
    let init_password_entry = widgets.init_password_entry.clone();
    let init_pending_spinner = widgets.init_pending_spinner.clone();
    let submit_button = widgets.submit_button.clone();
    let submit_spinner = widgets.submit_spinner.clone();
    let error_label = widgets.error_label.clone();
    let button_label = widgets.button_label.clone();
    let word_labels = widgets.word_labels.clone();
    let init_verify_hint_label = widgets.init_verify_hint_label.clone();
    let init_verify_a_label = widgets.init_verify_a_label.clone();
    let init_verify_b_label = widgets.init_verify_b_label.clone();
    let gen_key_fn_for_handler = gen_key_fn.clone();
    let do_bootstrap_fn_for_handler = do_bootstrap_fn.clone();
    let window_for_handler = window.clone();
    let authenticated_for_handler = Rc::clone(&authenticated);
    let on_authenticated_for_handler = Rc::clone(&on_authenticated);
    let init_oath_words_for_handler = Rc::clone(&init_oath_words);
    let init_verify_indices_for_handler = Rc::clone(&init_verify_indices);
    let init_clipboard_dirty_for_handler = Rc::clone(&init_clipboard_dirty);
    let init_clipboard_timer_for_handler = Rc::clone(&init_clipboard_timer);

    widgets.submit_button.connect_clicked(move |_| {
        // Vérifier l'étape courante
        let child_name = step_stack.visible_child_name();
        let current_step = child_name
            .as_ref()
            .map(|n| n.as_str())
            .unwrap_or("");

        match current_step {
            "init-identity" => {
                // Étape 1: Générer la clé de récupération et passer à l'étape oath
                // gen_key_fn_for_handler est Option<Arc<...>>, on passe &gen_key_fn_for_handler
                let gen_key_ref: Option<&Arc<dyn Fn() -> Result<String, AppError> + Send + Sync>> = 
                    gen_key_fn_for_handler.as_ref();
                handle_init_identity_step(
                    &init_username_entry,
                    &error_label,
                    gen_key_ref,
                    &word_labels,
                    Rc::clone(&init_verify_indices_for_handler),
                    &init_verify_hint_label,
                    &init_verify_a_label,
                    &init_verify_b_label,
                    Rc::clone(&init_oath_words_for_handler),
                    &Rc::new(move || {}), // _check_init_identity_gate - pas utilisé ici car on passe à oath
                    &step_stack,
                    &submit_button,
                );
                // Mettre à jour le texte du bouton pour l'étape oath
                button_label.set_text(heelonvault_core::tr!("init-confirm-button").as_str());
                submit_button.add_css_class("suggested-action");
            }
            "init-oath" => {
                // Étape 2: Vérifier les mots et exécuter le bootstrap
                handle_init_oath_step(
                    Rc::clone(&init_clipboard_dirty_for_handler),
                    Rc::clone(&init_clipboard_timer_for_handler),
                    &init_username_entry,
                    &init_password_entry,
                    &step_stack,
                    &init_pending_spinner,
                    do_bootstrap_fn_for_handler.clone(),
                    &window_for_handler,
                    &error_label,
                    &submit_button,
                    &submit_spinner,
                    Rc::clone(&authenticated_for_handler),
                    Rc::clone(&on_authenticated_for_handler),
                );
            }
            "init-pending" => {
                // En attente, ne rien faire
            }
            _ => {
                // Cas normal (ne devrait pas arriver en mode bootstrap)
            }
        }
    });
}

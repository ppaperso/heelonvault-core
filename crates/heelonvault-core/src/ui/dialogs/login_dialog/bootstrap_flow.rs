use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;

use crate::errors::AppError;
use crate::i18n::I18nArg;
use crate::services::admin_service::BootstrapResult;

use super::{feedback, AuthenticatedSession};

type BootstrapCallback =
    Arc<dyn Fn(String, Vec<u8>) -> Result<BootstrapResult, AppError> + Send + Sync>;

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
    check_init_identity_gate: &Rc<impl Fn()>,
    step_stack: &gtk4::Stack,
) {
    let username = init_username.text().trim().to_string();
    if username.is_empty() {
        feedback::show_feedback(
            error_label,
            crate::tr!("init-error-username-empty").as_str(),
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
                    let phrase_sum: u64 = phrase.bytes().map(|value| value as u64).sum();
                    let index_a = (phrase_sum % 24) as usize;
                    let index_b_raw = ((phrase_sum / 24).wrapping_add(7) % 24) as usize;
                    let index_b = if index_b_raw == index_a {
                        (index_a + 1) % 24
                    } else {
                        index_b_raw
                    };
                    let (index_a, index_b) = (index_a.min(index_b), index_a.max(index_b));
                    init_verify_indices.set((index_a, index_b));

                    let hint = crate::i18n::tr_args(
                        "init-verify-hint",
                        &[
                            ("a", I18nArg::Num((index_a + 1) as i64)),
                            ("b", I18nArg::Num((index_b + 1) as i64)),
                        ],
                    );
                    init_verify_hint.set_text(hint.as_str());

                    let label_a = crate::i18n::tr_args(
                        "init-verify-label-a",
                        &[("index", I18nArg::Num((index_a + 1) as i64))],
                    );
                    init_verify_a_label.set_text(label_a.as_str());

                    let label_b = crate::i18n::tr_args(
                        "init-verify-label-b",
                        &[("index", I18nArg::Num((index_b + 1) as i64))],
                    );
                    init_verify_b_label.set_text(label_b.as_str());

                    *init_oath_words.borrow_mut() = words;
                    check_init_identity_gate();
                    step_stack.set_visible_child_name("init-oath");
                } else {
                    feedback::show_feedback(
                        error_label,
                        crate::tr!("login-error-internal").as_str(),
                    );
                }
            }
            Err(_) => {
                feedback::show_feedback(error_label, crate::tr!("login-error-internal").as_str());
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
                    AppError::Conflict(_) => crate::tr!("init-error-already-initialized"),
                    _ => crate::tr!("login-error-unavailable"),
                };
                feedback::show_feedback(&error_for_result, message.as_str());
            }
            Err(_) => {
                spinner_result.stop();
                step_for_result.set_visible_child_name("init-identity");
                feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                feedback::show_feedback(
                    &error_for_result,
                    crate::tr!("login-error-interrupted").as_str(),
                );
            }
        }
    });
}

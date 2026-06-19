use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use secrecy::SecretBox;
use tokio::runtime::Handle;

use crate::ui::messages;
use heelonvault_core::errors::AppError;
use heelonvault_core::services::auth_policy_service::AuthPolicyService;
use heelonvault_core::services::auth_service::AuthService;
use heelonvault_core::services::totp_service::TotpService;
use heelonvault_core::services::user_service::UserService;

use super::{AuthenticatedSession, LoginAttemptOutcome, feedback, lock_state};

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_totp_submit<TAuth, TPolicy, TUser, TTotp>(
    runtime: Handle,
    auth_service: Arc<TAuth>,
    auth_policy_service: Arc<TPolicy>,
    user_service: Arc<TUser>,
    totp_service: Arc<TTotp>,
    username_entry: &gtk4::Entry,
    password_entry: &gtk4::PasswordEntry,
    totp_entry: &gtk4::Entry,
    submit_button: &gtk4::Button,
    submit_spinner: &gtk4::Spinner,
    dialog: &gtk4::Window,
    error_label: &gtk4::Label,
    authenticated: Rc<Cell<bool>>,
    on_authenticated: Rc<dyn Fn(AuthenticatedSession)>,
    lock_active: Rc<Cell<bool>>,
    lock_timer: Rc<RefCell<Option<glib::SourceId>>>,
    failure_cooldown_ms: u64,
) where
    TAuth: AuthService + Send + Sync + 'static,
    TPolicy: AuthPolicyService + Send + Sync + 'static,
    TUser: UserService + Send + Sync + 'static,
    TTotp: TotpService + Send + Sync + 'static,
{
    let totp = totp_entry.text().trim().to_string();
    let username = username_entry.text().trim().to_string();
    let password = password_entry.text().to_string();

    feedback::set_pending_state(submit_button, submit_spinner, true);

    let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
    let auth_for_task = Arc::clone(&auth_service);
    let auth_policy_for_task = Arc::clone(&auth_policy_service);
    let user_for_task = Arc::clone(&user_service);
    let totp_for_task = Arc::clone(&totp_service);
    let username_for_task = username;
    let password_for_task = password.into_bytes();
    let totp_for_task_value = totp.clone();
    let runtime_for_task = runtime.clone();

    std::thread::spawn(move || {
        let password_bytes = password_for_task;
        let result: Result<LoginAttemptOutcome, AppError> = runtime_for_task.block_on(async move {
            let resolved_username = user_for_task
                .resolve_username_for_login_identifier(&username_for_task)
                .await?;

            let canonical_username = match resolved_username {
                Some(value) => value,
                None => {
                    return Ok(LoginAttemptOutcome::InvalidCredentials {
                        remaining_lock_secs: 0,
                    });
                }
            };

            let lock_state_result = auth_policy_for_task
                .get_state(canonical_username.as_str())
                .await?;
            if lock_state_result.is_locked() {
                return Ok(LoginAttemptOutcome::Locked {
                    remaining_lock_secs: lock_state_result.remaining_lock_secs,
                });
            }

            if !feedback::is_valid_totp(&totp_for_task_value) {
                let state = auth_policy_for_task
                    .record_failed_attempt(canonical_username.as_str())
                    .await?;
                return Ok(LoginAttemptOutcome::InvalidTotp {
                    remaining_lock_secs: state.remaining_lock_secs,
                });
            }

            let totp_ok = totp_for_task
                .verify_login_totp(
                    canonical_username.as_str(),
                    SecretBox::new(Box::new(password_bytes.clone())),
                    totp_for_task_value.as_str(),
                )
                .await?;

            if !totp_ok {
                let state = auth_policy_for_task
                    .record_failed_attempt(canonical_username.as_str())
                    .await?;
                return Ok(LoginAttemptOutcome::InvalidTotp {
                    remaining_lock_secs: state.remaining_lock_secs,
                });
            }

            let master_key_opt = auth_for_task
                .derive_key_if_valid(
                    canonical_username.as_str(),
                    SecretBox::new(Box::new(password_bytes.clone())),
                )
                .await?;
            let Some(master_key) = master_key_opt else {
                let state = auth_policy_for_task
                    .record_failed_attempt(canonical_username.as_str())
                    .await?;
                return Ok(LoginAttemptOutcome::InvalidCredentials {
                    remaining_lock_secs: state.remaining_lock_secs,
                });
            };

            let user_profile = user_for_task
                .get_user_profile_by_username(canonical_username.as_str())
                .await?;

            let identity_label = user_profile
                .display_name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.to_string())
                .unwrap_or_else(|| user_profile.username.clone());

            auth_policy_for_task
                .reset_failed_attempts(canonical_username.as_str())
                .await?;
            Ok(LoginAttemptOutcome::Success(AuthenticatedSession {
                user_id: user_profile.id,
                username: canonical_username,
                identity_label,
                master_key,
            }))
        });
        let _ = result_sender.send(result);
    });

    let dialog_for_result = dialog.clone();
    let error_for_result = error_label.clone();
    let button_for_result = submit_button.clone();
    let spinner_for_result = submit_spinner.clone();
    let totp_for_result = totp_entry.clone();
    let authenticated_for_result = Rc::clone(&authenticated);
    let on_authenticated_for_result = Rc::clone(&on_authenticated);
    let lock_active_for_result = Rc::clone(&lock_active);
    let lock_timer_for_result = Rc::clone(&lock_timer);

    glib::MainContext::default().spawn_local(async move {
        let verification_result = result_receiver.await;

        match verification_result {
            Ok(Ok(LoginAttemptOutcome::Success(session))) => {
                feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                authenticated_for_result.set(true);
                on_authenticated_for_result(session);
                dialog_for_result.close();
            }
            Ok(Ok(LoginAttemptOutcome::InvalidTotp {
                remaining_lock_secs,
            })) => {
                totp_for_result.grab_focus();

                if remaining_lock_secs > 0 {
                    feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                    lock_state::start_lock_countdown(
                        &button_for_result,
                        &spinner_for_result,
                        &error_for_result,
                        remaining_lock_secs,
                        Rc::clone(&lock_active_for_result),
                        Rc::clone(&lock_timer_for_result),
                        feedback::set_pending_state,
                        feedback::show_feedback,
                    );
                } else {
                    let totp_feedback =
                        messages::login_totp_error_message(totp_for_result.text().trim());
                    feedback::show_feedback(&error_for_result, totp_feedback.as_str());
                    let button_after_delay = button_for_result.clone();
                    let spinner_after_delay = spinner_for_result.clone();
                    glib::timeout_add_local_once(
                        Duration::from_millis(failure_cooldown_ms),
                        move || {
                            feedback::set_pending_state(
                                &button_after_delay,
                                &spinner_after_delay,
                                false,
                            );
                        },
                    );
                }
            }
            Ok(Ok(LoginAttemptOutcome::Locked {
                remaining_lock_secs,
            })) => {
                feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                lock_state::start_lock_countdown(
                    &button_for_result,
                    &spinner_for_result,
                    &error_for_result,
                    remaining_lock_secs,
                    Rc::clone(&lock_active_for_result),
                    Rc::clone(&lock_timer_for_result),
                    feedback::set_pending_state,
                    feedback::show_feedback,
                );
            }
            Ok(Ok(LoginAttemptOutcome::InvalidCredentials {
                remaining_lock_secs,
            })) => {
                if remaining_lock_secs > 0 {
                    feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                    lock_state::start_lock_countdown(
                        &button_for_result,
                        &spinner_for_result,
                        &error_for_result,
                        remaining_lock_secs,
                        Rc::clone(&lock_active_for_result),
                        Rc::clone(&lock_timer_for_result),
                        feedback::set_pending_state,
                        feedback::show_feedback,
                    );
                } else {
                    feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
                    feedback::show_feedback(
                        &error_for_result,
                        heelonvault_core::tr!("login-error-credentials").as_str(),
                    );
                }
            }
            Ok(Ok(LoginAttemptOutcome::RequiresTotp)) => {
                feedback::set_pending_state(&button_for_result, &spinner_for_result, false);
            }
            Ok(Err(_)) => {
                feedback::show_feedback(
                    &error_for_result,
                    heelonvault_core::tr!("login-error-unavailable").as_str(),
                );
                let button_after_delay = button_for_result.clone();
                let spinner_after_delay = spinner_for_result.clone();
                glib::timeout_add_local_once(
                    Duration::from_millis(failure_cooldown_ms),
                    move || {
                        feedback::set_pending_state(
                            &button_after_delay,
                            &spinner_after_delay,
                            false,
                        );
                    },
                );
            }
            Err(_) => {
                feedback::show_feedback(
                    &error_for_result,
                    heelonvault_core::tr!("login-error-interrupted").as_str(),
                );
                let button_after_delay = button_for_result.clone();
                let spinner_after_delay = spinner_for_result.clone();
                glib::timeout_add_local_once(
                    Duration::from_millis(failure_cooldown_ms),
                    move || {
                        feedback::set_pending_state(
                            &button_after_delay,
                            &spinner_after_delay,
                            false,
                        );
                    },
                );
            }
        }
    });
}

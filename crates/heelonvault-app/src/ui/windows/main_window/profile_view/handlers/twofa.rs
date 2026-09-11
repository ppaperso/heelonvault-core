//! TOTP activation, confirmation and deactivation handlers.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use tokio::runtime::Handle;
use uuid::Uuid;

use heelonvault_core::services::totp_service::TotpService;

use super::super::sections::identity::IdentitySection;
use super::super::sections::twofa::TwoFaSection;
use crate::ui::messages;
use crate::ui::windows::main_window::MainWindow;

pub struct TwoFaHandlerDeps<TTotp> {
    pub runtime_handle: Handle,
    pub totp_service: Arc<TTotp>,
    pub user_id: Uuid,
}

/// Wire the whole TOTP lifecycle: activate, confirm, cancel and disable.
///
/// The identity section is needed because the provisioning URI is labelled with the
/// account's username.
pub fn setup<TTotp>(
    twofa_section: &TwoFaSection,
    identity_section: &IdentitySection,
    deps: TwoFaHandlerDeps<TTotp>,
) where
    TTotp: TotpService + Send + Sync + 'static,
{
    let TwoFaHandlerDeps {
        runtime_handle,
        totp_service,
        user_id,
    } = deps;

    let pending_totp_secret: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let username_for_twofa_activate = identity_section.username_entry.clone();
    let twofa_stack_for_activate = twofa_section.stack.clone();
    let twofa_qr_for_activate = twofa_section.qr_picture.clone();
    let twofa_secret_for_activate = twofa_section.secret_entry.clone();
    let twofa_code_for_activate = twofa_section.code_entry.clone();
    let twofa_status_for_activate = twofa_section.status_label.clone();
    let pending_secret_for_activate = Rc::clone(&pending_totp_secret);
    let totp_for_activate = Arc::clone(&totp_service);
    twofa_section.activate_button.connect_clicked(move |_| {
        let username = username_for_twofa_activate.text().trim().to_string();
        if username.is_empty() {
            MainWindow::set_inline_status(
                &twofa_status_for_activate,
                heelonvault_core::tr!("profile-status-twofa-profile-missing-start").as_str(),
                "error",
            );
            return;
        }

        match totp_for_activate.create_setup_payload(username.as_str()) {
            Ok(payload) => {
                *pending_secret_for_activate.borrow_mut() = Some(payload.base32_secret.clone());
                let loader = gtk4::gdk_pixbuf::PixbufLoader::new();
                if loader.write(&payload.qr_png).is_ok()
                    && loader.close().is_ok()
                    && let Some(pixbuf) = loader.pixbuf()
                {
                    let texture = gtk4::gdk::Texture::for_pixbuf(&pixbuf);
                    twofa_qr_for_activate.set_paintable(Some(&texture));
                }
                twofa_secret_for_activate.set_text(payload.base32_secret.as_str());
                twofa_code_for_activate.set_text("");
                twofa_stack_for_activate.set_visible_child_name("setup");
                MainWindow::set_inline_status(
                    &twofa_status_for_activate,
                    heelonvault_core::tr!("profile-status-twofa-ready").as_str(),
                    "success",
                );
            }
            Err(error) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_activate,
                    MainWindow::map_twofa_error(
                        &error,
                        heelonvault_core::tr!("profile-status-twofa-prepare-failed").as_str(),
                    )
                    .as_str(),
                    "error",
                );
            }
        }
    });

    let pending_secret_for_cancel = Rc::clone(&pending_totp_secret);
    let twofa_stack_for_cancel = twofa_section.stack.clone();
    let twofa_status_for_cancel = twofa_section.status_label.clone();
    twofa_section.cancel_setup_button.connect_clicked(move |_| {
        *pending_secret_for_cancel.borrow_mut() = None;
        twofa_stack_for_cancel.set_visible_child_name("disabled");
        MainWindow::set_inline_status(
            &twofa_status_for_cancel,
            heelonvault_core::tr!("profile-status-twofa-cancelled").as_str(),
            "success",
        );
    });

    let username_for_twofa_confirm = identity_section.username_entry.clone();
    let twofa_code_for_confirm = twofa_section.code_entry.clone();
    let twofa_stack_for_confirm = twofa_section.stack.clone();
    let twofa_status_for_confirm = twofa_section.status_label.clone();
    let twofa_badge_for_confirm = twofa_section.state_badge.clone();
    let pending_secret_for_confirm = Rc::clone(&pending_totp_secret);
    let totp_for_confirm = Arc::clone(&totp_service);
    let runtime_for_twofa_confirm = runtime_handle.clone();
    twofa_section.confirm_button.connect_clicked(move |_| {
        let username = username_for_twofa_confirm.text().trim().to_string();
        let code = twofa_code_for_confirm.text().trim().to_string();

        if username.is_empty() {
            MainWindow::set_inline_status(
                &twofa_status_for_confirm,
                heelonvault_core::tr!("profile-status-twofa-profile-missing-finish").as_str(),
                "error",
            );
            return;
        }

        let Some(secret) = pending_secret_for_confirm.borrow().clone() else {
            MainWindow::set_inline_status(
                &twofa_status_for_confirm,
                heelonvault_core::tr!("profile-status-twofa-none-pending").as_str(),
                "error",
            );
            return;
        };

        if let Some(validation_message) = messages::validate_totp_code_format(code.as_str()) {
            MainWindow::set_inline_status(
                &twofa_status_for_confirm,
                validation_message.as_str(),
                "error",
            );
            return;
        }

        match totp_for_confirm.verify_setup_code(username.as_str(), secret.as_str(), code.as_str())
        {
            Ok(true) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_confirm,
                    heelonvault_core::tr!("profile-status-twofa-enabling").as_str(),
                    "loading",
                );

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let runtime_for_task = runtime_for_twofa_confirm.clone();
                let totp_for_task = Arc::clone(&totp_for_confirm);
                std::thread::spawn(move || {
                    let result = runtime_for_task.block_on(async move {
                        totp_for_task
                            .enable_totp(user_id, username.as_str(), secret.as_str(), code.as_str())
                            .await
                    });
                    let _ = sender.send(result);
                });

                let twofa_stack_for_result = twofa_stack_for_confirm.clone();
                let twofa_status_for_result = twofa_status_for_confirm.clone();
                let twofa_badge_for_result = twofa_badge_for_confirm.clone();
                let twofa_code_for_result = twofa_code_for_confirm.clone();
                let pending_secret_for_result = Rc::clone(&pending_secret_for_confirm);
                glib::MainContext::default().spawn_local(async move {
                    match receiver.await {
                        Ok(Ok(())) => {
                            *pending_secret_for_result.borrow_mut() = None;
                            twofa_code_for_result.set_text("");
                            twofa_stack_for_result.set_visible_child_name("enabled");
                            MainWindow::set_twofa_badge_state(&twofa_badge_for_result, true);
                            MainWindow::set_inline_status(
                                &twofa_status_for_result,
                                heelonvault_core::tr!("profile-status-twofa-enabled").as_str(),
                                "success",
                            );
                        }
                        Ok(Err(error)) => {
                            MainWindow::set_inline_status(
                                &twofa_status_for_result,
                                MainWindow::map_twofa_error(
                                    &error,
                                    heelonvault_core::tr!("profile-status-twofa-enable-failed")
                                        .as_str(),
                                )
                                .as_str(),
                                "error",
                            );
                        }
                        Err(_) => {
                            MainWindow::set_inline_status(
                                &twofa_status_for_result,
                                heelonvault_core::tr!("profile-status-twofa-interrupted").as_str(),
                                "error",
                            );
                        }
                    }
                });
            }
            Ok(false) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_confirm,
                    messages::profile_totp_code_invalid_error().as_str(),
                    "error",
                );
            }
            Err(error) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_confirm,
                    MainWindow::map_twofa_error(
                        &error,
                        heelonvault_core::tr!("profile-status-twofa-verify-failed").as_str(),
                    )
                    .as_str(),
                    "error",
                );
            }
        }
    });

    let twofa_confirm_row_for_toggle = twofa_section.disable_confirm_row.clone();
    twofa_section
        .disable_toggle_button
        .connect_clicked(move |_| {
            twofa_confirm_row_for_toggle.set_visible(true);
        });

    let twofa_confirm_row_for_cancel = twofa_section.disable_confirm_row.clone();
    twofa_section
        .disable_cancel_button
        .connect_clicked(move |_| {
            twofa_confirm_row_for_cancel.set_visible(false);
        });

    let totp_for_disable = Arc::clone(&totp_service);
    let runtime_for_disable = runtime_handle.clone();
    let twofa_stack_for_disable = twofa_section.stack.clone();
    let twofa_status_for_disable = twofa_section.status_label.clone();
    let twofa_badge_for_disable = twofa_section.state_badge.clone();
    let twofa_confirm_row_for_disable = twofa_section.disable_confirm_row.clone();
    twofa_section
        .disable_confirm_button
        .connect_clicked(move |_| {
            MainWindow::set_inline_status(
                &twofa_status_for_disable,
                heelonvault_core::tr!("profile-status-twofa-disabling").as_str(),
                "loading",
            );

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let runtime_for_task = runtime_for_disable.clone();
            let totp_for_task = Arc::clone(&totp_for_disable);
            std::thread::spawn(move || {
                let result = runtime_for_task
                    .block_on(async move { totp_for_task.disable_totp(user_id).await });
                let _ = sender.send(result);
            });

            let twofa_stack_for_result = twofa_stack_for_disable.clone();
            let twofa_status_for_result = twofa_status_for_disable.clone();
            let twofa_badge_for_result = twofa_badge_for_disable.clone();
            let twofa_confirm_row_for_result = twofa_confirm_row_for_disable.clone();
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(())) => {
                        twofa_confirm_row_for_result.set_visible(false);
                        twofa_stack_for_result.set_visible_child_name("disabled");
                        MainWindow::set_twofa_badge_state(&twofa_badge_for_result, false);
                        MainWindow::set_inline_status(
                            &twofa_status_for_result,
                            heelonvault_core::tr!("profile-status-twofa-disabled").as_str(),
                            "success",
                        );
                    }
                    Ok(Err(error)) => {
                        MainWindow::set_inline_status(
                            &twofa_status_for_result,
                            MainWindow::map_twofa_error(
                                &error,
                                heelonvault_core::tr!("profile-status-twofa-disable-failed")
                                    .as_str(),
                            )
                            .as_str(),
                            "error",
                        );
                    }
                    Err(_) => {
                        MainWindow::set_inline_status(
                            &twofa_status_for_result,
                            heelonvault_core::tr!("profile-status-twofa-interrupted").as_str(),
                            "error",
                        );
                    }
                }
            });
        });
}

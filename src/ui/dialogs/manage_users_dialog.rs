use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use tokio::runtime::Handle;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::UserRole;
use crate::services::admin_service::AdminService;
use crate::services::vault_service::VaultService;

pub struct ManageUsersDialog {
    window: gtk4::Window,
}

impl ManageUsersDialog {
    pub fn new<TAdmin, TVault>(
        application: &adw::Application,
        parent: &adw::ApplicationWindow,
        runtime_handle: Handle,
        admin_service: Arc<TAdmin>,
        vault_service: Arc<TVault>,
        actor_user_id: Uuid,
    ) -> Self
    where
        TAdmin: AdminService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        let window = gtk4::Window::builder()
            .application(application)
            .transient_for(parent)
            .title(crate::tr!("manage-users-window-title").as_str())
            .modal(true)
            .default_width(760)
            .default_height(560)
            .build();

        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let header = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();
        let title = gtk4::Label::new(Some(crate::tr!("manage-users-title").as_str()));
        title.set_halign(gtk4::Align::Start);
        title.set_hexpand(true);
        title.add_css_class("title-3");

        let create_button =
            gtk4::Button::with_label(crate::tr!("manage-users-create-button").as_str());
        create_button.add_css_class("suggested-action");
        header.append(&title);
        header.append(&create_button);

        let columns = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();
        columns.add_css_class("dim-label");
        let col_user = gtk4::Label::new(Some(crate::tr!("manage-users-col-username").as_str()));
        col_user.set_hexpand(true);
        col_user.set_halign(gtk4::Align::Start);
        let col_role = gtk4::Label::new(Some(crate::tr!("manage-users-col-role").as_str()));
        col_role.set_width_chars(10);
        col_role.set_halign(gtk4::Align::Start);
        let col_date = gtk4::Label::new(Some(crate::tr!("manage-users-col-created-at").as_str()));
        col_date.set_width_chars(20);
        col_date.set_halign(gtk4::Align::Start);
        columns.append(&col_user);
        columns.append(&col_role);
        columns.append(&col_date);

        let scrolled = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk4::SelectionMode::None);
        scrolled.set_child(Some(&list));

        root.append(&header);
        root.append(&columns);
        root.append(&scrolled);
        window.set_child(Some(&root));

        let refresh: Rc<dyn Fn()> = {
            let list_for_refresh = list.clone();
            let runtime_for_refresh = runtime_handle.clone();
            let admin_for_refresh = Arc::clone(&admin_service);
            let window_for_refresh = window.clone();
            Rc::new(move || {
                while let Some(child) = list_for_refresh.first_child() {
                    list_for_refresh.remove(&child);
                }

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let runtime_for_task = runtime_for_refresh.clone();
                let admin_for_task = Arc::clone(&admin_for_refresh);
                std::thread::spawn(move || {
                    let result = runtime_for_task.block_on(async move {
                        admin_for_task.list_all_users(actor_user_id).await
                    });
                    let _ = sender.send(result);
                });

                let list_for_recv = list_for_refresh.clone();
                let runtime_for_recv = runtime_for_refresh.clone();
                let admin_for_recv = Arc::clone(&admin_for_refresh);
                let window_for_recv = window_for_refresh.clone();
                glib::MainContext::default().spawn_local(async move {
                    match receiver.await {
                        Ok(Ok(users)) => {
                            for user in users {
                                let row_box = gtk4::Box::builder()
                                    .orientation(gtk4::Orientation::Horizontal)
                                    .spacing(8)
                                    .margin_top(8)
                                    .margin_bottom(8)
                                    .margin_start(8)
                                    .margin_end(8)
                                    .build();

                                let username = gtk4::Label::new(Some(user.username.as_str()));
                                username.set_hexpand(true);
                                username.set_halign(gtk4::Align::Start);
                                let role_text = match user.role {
                                    UserRole::Admin => {
                                        crate::tr!("manage-users-role-admin").to_string()
                                    }
                                    UserRole::User => {
                                        crate::tr!("manage-users-role-user").to_string()
                                    }
                                };
                                let role = gtk4::Label::new(Some(role_text.as_str()));
                                role.set_width_chars(10);
                                role.set_halign(gtk4::Align::Start);
                                let created = gtk4::Label::new(Some(
                                    user.updated_at.as_deref().unwrap_or("-"),
                                ));
                                created.set_width_chars(20);
                                created.set_halign(gtk4::Align::Start);

                                let reset_btn = gtk4::Button::with_label(
                                    crate::tr!("manage-users-reset-password").as_str(),
                                );
                                reset_btn.add_css_class("flat");
                                let toggle_label = if matches!(user.role, UserRole::Admin) {
                                    crate::tr!("manage-users-toggle-to-user").to_string()
                                } else {
                                    crate::tr!("manage-users-toggle-to-admin").to_string()
                                };
                                let toggle_btn = gtk4::Button::with_label(toggle_label.as_str());
                                toggle_btn.add_css_class("flat");
                                let delete_btn = gtk4::Button::with_label(
                                    crate::tr!("manage-users-delete").as_str(),
                                );
                                delete_btn.add_css_class("destructive-action");

                                row_box.append(&username);
                                row_box.append(&role);
                                row_box.append(&created);
                                row_box.append(&reset_btn);
                                row_box.append(&toggle_btn);
                                row_box.append(&delete_btn);

                                let row = gtk4::ListBoxRow::new();
                                row.set_child(Some(&row_box));
                                list_for_recv.append(&row);

                                let runtime_for_toggle = runtime_for_recv.clone();
                                let admin_for_toggle = Arc::clone(&admin_for_recv);
                                let window_for_toggle = window_for_recv.clone();
                                let refresh_toggle = {
                                    let list_for_refresh_again = list_for_recv.clone();
                                    let runtime_for_refresh_again = runtime_for_recv.clone();
                                    let admin_for_refresh_again = Arc::clone(&admin_for_recv);
                                    let window_for_refresh_again = window_for_recv.clone();
                                    Rc::new(move || {
                                        while let Some(child) = list_for_refresh_again.first_child()
                                        {
                                            list_for_refresh_again.remove(&child);
                                        }
                                        let (sender, receiver) = tokio::sync::oneshot::channel();
                                        let runtime_for_task = runtime_for_refresh_again.clone();
                                        let admin_for_task = Arc::clone(&admin_for_refresh_again);
                                        std::thread::spawn(move || {
                                            let result = runtime_for_task.block_on(async move {
                                                admin_for_task.list_all_users(actor_user_id).await
                                            });
                                            let _ = sender.send(result);
                                        });
                                        let list_for_recv2 = list_for_refresh_again.clone();
                                        let window_for_err = window_for_refresh_again.clone();
                                        glib::MainContext::default().spawn_local(async move {
                                            if let Ok(Err(err)) | Err(err) =
                                                receiver.await.map_err(|_| AppError::Internal)
                                            {
                                                Self::show_error(Some(&window_for_err), err);
                                            }
                                            let _ = &list_for_recv2;
                                        });
                                    })
                                };

                                let target_user_id = user.id;
                                let target_role = if matches!(user.role, UserRole::Admin) {
                                    UserRole::User
                                } else {
                                    UserRole::Admin
                                };
                                let refresh_toggle_for_toggle = refresh_toggle.clone();
                                toggle_btn.connect_clicked(move |_| {
                                    let (sender, receiver) = tokio::sync::oneshot::channel();
                                    let runtime_for_task = runtime_for_toggle.clone();
                                    let admin_for_task = Arc::clone(&admin_for_toggle);
                                    let role_for_task = target_role.clone();
                                    std::thread::spawn(move || {
                                        let result = runtime_for_task.block_on(async move {
                                            admin_for_task
                                                .update_user_role(
                                                    actor_user_id,
                                                    target_user_id,
                                                    role_for_task,
                                                )
                                                .await
                                        });
                                        let _ = sender.send(result);
                                    });

                                    let window_for_result = window_for_toggle.clone();
                                    let refresh_after = refresh_toggle_for_toggle.clone();
                                    glib::MainContext::default().spawn_local(async move {
                                        match receiver.await {
                                            Ok(Ok(())) => refresh_after(),
                                            Ok(Err(err)) => {
                                                Self::show_error(Some(&window_for_result), err)
                                            }
                                            Err(_) => Self::show_error(
                                                Some(&window_for_result),
                                                AppError::Internal,
                                            ),
                                        }
                                    });
                                });

                                let runtime_for_delete = runtime_for_recv.clone();
                                let admin_for_delete = Arc::clone(&admin_for_recv);
                                let window_for_delete = window_for_recv.clone();
                                let refresh_delete = refresh_toggle.clone();
                                delete_btn.connect_clicked(move |_| {
                                    let confirm = adw::MessageDialog::new(
                                        Some(&window_for_delete),
                                        Some(
                                            crate::tr!("manage-users-delete-confirm-title")
                                                .as_str(),
                                        ),
                                        Some(
                                            crate::tr!("manage-users-delete-confirm-body").as_str(),
                                        ),
                                    );
                                    confirm.add_response(
                                        "cancel",
                                        crate::tr!("common-cancel").as_str(),
                                    );
                                    confirm.add_response(
                                        "delete",
                                        crate::tr!("manage-users-delete").as_str(),
                                    );
                                    confirm.set_response_appearance(
                                        "delete",
                                        adw::ResponseAppearance::Destructive,
                                    );
                                    let admin_for_resp = Arc::clone(&admin_for_delete);
                                    let runtime_for_resp = runtime_for_delete.clone();
                                    let window_for_resp = window_for_delete.clone();
                                    let refresh_after = refresh_delete.clone();
                                    confirm.connect_response(Some("delete"), move |d, _| {
                                        d.close();
                                        let (sender, receiver) = tokio::sync::oneshot::channel();
                                        let runtime_for_task = runtime_for_resp.clone();
                                        let admin_for_task = Arc::clone(&admin_for_resp);
                                        std::thread::spawn(move || {
                                            let result = runtime_for_task.block_on(async move {
                                                admin_for_task
                                                    .delete_user(actor_user_id, target_user_id)
                                                    .await
                                            });
                                            let _ = sender.send(result);
                                        });
                                        let window_for_result = window_for_resp.clone();
                                        let refresh_after_result = refresh_after.clone();
                                        glib::MainContext::default().spawn_local(async move {
                                            match receiver.await {
                                                Ok(Ok(())) => refresh_after_result(),
                                                Ok(Err(err)) => {
                                                    Self::show_error(Some(&window_for_result), err)
                                                }
                                                Err(_) => Self::show_error(
                                                    Some(&window_for_result),
                                                    AppError::Internal,
                                                ),
                                            }
                                        });
                                    });
                                    confirm.present();
                                });

                                let runtime_for_reset = runtime_for_recv.clone();
                                let admin_for_reset = Arc::clone(&admin_for_recv);
                                let window_for_reset = window_for_recv.clone();
                                reset_btn.connect_clicked(move |_| {
                                    let reset_window = gtk4::Window::builder()
                                        .title(
                                            crate::tr!("manage-users-reset-window-title").as_str(),
                                        )
                                        .modal(true)
                                        .transient_for(&window_for_reset)
                                        .default_width(420)
                                        .default_height(120)
                                        .build();
                                    let box_root = gtk4::Box::builder()
                                        .orientation(gtk4::Orientation::Vertical)
                                        .spacing(8)
                                        .margin_top(12)
                                        .margin_bottom(12)
                                        .margin_start(12)
                                        .margin_end(12)
                                        .build();
                                    let password_entry = gtk4::PasswordEntry::new();
                                    password_entry.set_placeholder_text(Some(
                                        crate::tr!("manage-users-new-password-placeholder")
                                            .as_str(),
                                    ));
                                    let actions = gtk4::Box::builder()
                                        .orientation(gtk4::Orientation::Horizontal)
                                        .spacing(8)
                                        .halign(gtk4::Align::End)
                                        .build();
                                    let cancel = gtk4::Button::with_label(
                                        crate::tr!("common-cancel").as_str(),
                                    );
                                    let apply = gtk4::Button::with_label(
                                        crate::tr!("manage-users-apply").as_str(),
                                    );
                                    apply.add_css_class("suggested-action");
                                    actions.append(&cancel);
                                    actions.append(&apply);
                                    box_root.append(&password_entry);
                                    box_root.append(&actions);
                                    reset_window.set_child(Some(&box_root));

                                    let reset_window_for_cancel = reset_window.clone();
                                    cancel
                                        .connect_clicked(move |_| reset_window_for_cancel.close());

                                    let runtime_for_apply = runtime_for_reset.clone();
                                    let admin_for_apply = Arc::clone(&admin_for_reset);
                                    let window_for_apply = window_for_reset.clone();
                                    let reset_window_for_apply = reset_window.clone();
                                    apply.connect_clicked(move |_| {
                                        let password = password_entry.text().to_string();
                                        if password.trim().is_empty() {
                                            Self::show_error(
                                                Some(&window_for_apply),
                                                AppError::Validation(
                                                    crate::tr!("manage-users-error-empty-password")
                                                        .to_string(),
                                                ),
                                            );
                                            return;
                                        }
                                        let (sender, receiver) = tokio::sync::oneshot::channel();
                                        let runtime_for_task = runtime_for_apply.clone();
                                        let admin_for_task = Arc::clone(&admin_for_apply);
                                        std::thread::spawn(move || {
                                            let result = runtime_for_task.block_on(async move {
                                                admin_for_task
                                                    .reset_user_password(
                                                        actor_user_id,
                                                        target_user_id,
                                                        secrecy::SecretBox::new(Box::new(
                                                            password.into_bytes(),
                                                        )),
                                                    )
                                                    .await
                                            });
                                            let _ = sender.send(result);
                                        });
                                        let window_for_result = window_for_apply.clone();
                                        let reset_window_close = reset_window_for_apply.clone();
                                        glib::MainContext::default().spawn_local(async move {
                                            match receiver.await {
                                                Ok(Ok(_)) => reset_window_close.close(),
                                                Ok(Err(err)) => {
                                                    Self::show_error(Some(&window_for_result), err)
                                                }
                                                Err(_) => Self::show_error(
                                                    Some(&window_for_result),
                                                    AppError::Internal,
                                                ),
                                            }
                                        });
                                    });

                                    reset_window.present();
                                });
                            }
                        }
                        Ok(Err(err)) => Self::show_error(Some(&window_for_recv), err),
                        Err(_) => Self::show_error(Some(&window_for_recv), AppError::Internal),
                    }
                });
            })
        };

        {
            let runtime_for_create = runtime_handle.clone();
            let admin_for_create = Arc::clone(&admin_service);
            let vault_for_create = Arc::clone(&vault_service);
            let window_for_create = window.clone();
            let refresh_for_create = refresh.clone();
            create_button.connect_clicked(move |_| {
                let create_window = gtk4::Window::builder()
                    .title(crate::tr!("manage-users-create-window-title").as_str())
                    .modal(true)
                    .transient_for(&window_for_create)
                    .default_width(460)
                    .default_height(220)
                    .build();

                let root = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Vertical)
                    .spacing(10)
                    .margin_top(12)
                    .margin_bottom(12)
                    .margin_start(12)
                    .margin_end(12)
                    .build();

                let username = gtk4::Entry::new();
                username.set_placeholder_text(Some(
                    crate::tr!("manage-users-username-placeholder").as_str(),
                ));
                let password = gtk4::PasswordEntry::new();
                password.set_placeholder_text(Some(
                    crate::tr!("manage-users-password-placeholder").as_str(),
                ));
                let role = gtk4::DropDown::from_strings(&[
                    crate::tr!("manage-users-role-user").as_str(),
                    crate::tr!("manage-users-role-admin").as_str(),
                ]);
                role.set_selected(0);

                let actions = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Horizontal)
                    .spacing(8)
                    .halign(gtk4::Align::End)
                    .build();
                let cancel = gtk4::Button::with_label(crate::tr!("common-cancel").as_str());
                let create = gtk4::Button::with_label(crate::tr!("manage-users-create").as_str());
                create.add_css_class("suggested-action");
                actions.append(&cancel);
                actions.append(&create);

                root.append(&username);
                root.append(&password);
                root.append(&role);
                root.append(&actions);
                create_window.set_child(Some(&root));

                let create_window_for_cancel = create_window.clone();
                cancel.connect_clicked(move |_| create_window_for_cancel.close());

                let runtime_for_apply = runtime_for_create.clone();
                let admin_for_apply = Arc::clone(&admin_for_create);
                let vault_for_apply = Arc::clone(&vault_for_create);
                let window_for_apply = window_for_create.clone();
                let refresh_after = refresh_for_create.clone();
                let create_window_for_apply = create_window.clone();
                create.connect_clicked(move |_| {
                    let username_value = username.text().to_string();
                    let password_value = password.text().to_string();
                    let role_value = if role.selected() == 1 {
                        UserRole::Admin
                    } else {
                        UserRole::User
                    };

                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    let runtime_for_task = runtime_for_apply.clone();
                    let admin_for_task = Arc::clone(&admin_for_apply);
                    let vault_for_task = Arc::clone(&vault_for_apply);
                    std::thread::spawn(move || {
                        let result = runtime_for_task.block_on(async move {
                            let create_result = admin_for_task
                                .create_user(
                                    actor_user_id,
                                    username_value.as_str(),
                                    secrecy::SecretBox::new(Box::new(password_value.into_bytes())),
                                    role_value,
                                )
                                .await?;

                            let default_vault_name = "perso";
                            vault_for_task
                                .create_vault(
                                    create_result.user.id,
                                    default_vault_name,
                                    create_result.master_key,
                                )
                                .await?;

                            Ok::<(), AppError>(())
                        });
                        let _ = sender.send(result);
                    });

                    let window_for_result = window_for_apply.clone();
                    let create_window_close = create_window_for_apply.clone();
                    let refresh_after_result = refresh_after.clone();
                    glib::MainContext::default().spawn_local(async move {
                        match receiver.await {
                            Ok(Ok(_)) => {
                                create_window_close.close();
                                refresh_after_result();
                            }
                            Ok(Err(err)) => Self::show_error(Some(&window_for_result), err),
                            Err(_) => {
                                Self::show_error(Some(&window_for_result), AppError::Internal)
                            }
                        }
                    });
                });

                create_window.present();
            });
        }

        refresh();

        Self { window }
    }

    fn show_error(window: Option<&gtk4::Window>, err: AppError) {
        let title = match err {
            AppError::Authorization(_) => {
                crate::tr!("manage-users-error-authorization").to_string()
            }
            _ => crate::tr!("manage-users-error-generic").to_string(),
        };
        let message = err.to_string();
        let dialog = adw::MessageDialog::new(window, Some(title.as_str()), Some(message.as_str()));
        dialog.add_response("ok", crate::tr!("common-ok").as_str());
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        dialog.present();
    }

    pub fn take_content(&self) -> Option<gtk4::Widget> {
        let child = self.window.child();
        if child.is_some() {
            self.window.set_child(Option::<&gtk4::Widget>::None);
        }
        child
    }
}

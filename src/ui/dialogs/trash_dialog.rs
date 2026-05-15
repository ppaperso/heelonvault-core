use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use secrecy::SecretBox;
use serde_json::Value;
use tokio::runtime::Handle;
use uuid::Uuid;

use crate::services::secret_service::SecretService;
use crate::services::vault_service::VaultService;

pub struct TrashDialog {
    window: gtk4::Window,
}

impl TrashDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn new<TSecret, TVault>(
        application: &adw::Application,
        parent: &adw::ApplicationWindow,
        runtime_handle: Handle,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        admin_user_id: Uuid,
        admin_master_key: Vec<u8>,
        on_changed: impl Fn() + 'static,
    ) -> Self
    where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        let on_changed: Rc<dyn Fn()> = Rc::new(on_changed);

        let window = gtk4::Window::builder()
            .application(application)
            .transient_for(parent)
            .title(crate::tr!("trash-window-title").as_str())
            .modal(true)
            .default_width(760)
            .default_height(560)
            .build();
        window.add_css_class("app-window");

        let root = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let header = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();

        let title = gtk4::Label::new(Some(crate::tr!("trash-title").as_str()));
        title.add_css_class("title-3");
        title.set_halign(Align::Start);
        title.set_hexpand(true);

        let empty_button = gtk4::Button::with_label(crate::tr!("trash-empty-button").as_str());
        empty_button.add_css_class("destructive-action");

        header.append(&title);
        header.append(&empty_button);

        let stack = gtk4::Stack::builder()
            .vexpand(true)
            .hexpand(true)
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .build();

        let list_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .hexpand(true)
            .build();

        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        list_scroll.set_child(Some(&list));

        let empty_state = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .halign(Align::Center)
            .valign(Align::Center)
            .vexpand(true)
            .hexpand(true)
            .build();

        let empty_title = gtk4::Label::new(Some(crate::tr!("trash-empty-title").as_str()));
        empty_title.add_css_class("title-4");
        let empty_copy = gtk4::Label::new(Some(crate::tr!("trash-empty-description").as_str()));
        empty_copy.set_wrap(true);
        empty_copy.set_justify(gtk4::Justification::Center);
        empty_copy.set_max_width_chars(56);

        empty_state.append(&empty_title);
        empty_state.append(&empty_copy);

        stack.add_titled(
            &list_scroll,
            Some("list"),
            crate::tr!("trash-list-title").as_str(),
        );
        stack.add_titled(
            &empty_state,
            Some("empty"),
            crate::tr!("trash-list-empty").as_str(),
        );
        stack.set_visible_child_name("empty");

        root.append(&header);
        root.append(&stack);
        window.set_child(Some(&root));

        let refresh_trash: Rc<dyn Fn()> = {
            let runtime = runtime_handle.clone();
            let secret = Arc::clone(&secret_service);
            let vault = Arc::clone(&vault_service);
            let list_widget = list.clone();
            let stack_widget = stack.clone();
            let empty_title_widget = empty_title.clone();
            let empty_copy_widget = empty_copy.clone();
            let master = admin_master_key.clone();
            let app = application.clone();
            let parent_window = parent.clone();
            let on_changed_cb = Rc::clone(&on_changed);
            Rc::new(move || {
                Self::refresh_trash_list(
                    app.clone(),
                    parent_window.clone(),
                    runtime.clone(),
                    Arc::clone(&secret),
                    Arc::clone(&vault),
                    admin_user_id,
                    master.clone(),
                    list_widget.clone(),
                    stack_widget.clone(),
                    empty_title_widget.clone(),
                    empty_copy_widget.clone(),
                    Rc::clone(&on_changed_cb),
                );
            })
        };

        let refresh_for_empty = Rc::clone(&refresh_trash);
        let runtime_for_empty = runtime_handle.clone();
        let secret_for_empty = Arc::clone(&secret_service);
        let vault_for_empty = Arc::clone(&vault_service);
        let master_for_empty = admin_master_key.clone();
        let window_for_empty = window.clone();
        let on_changed_for_empty = Rc::clone(&on_changed);
        empty_button.connect_clicked(move |_| {
            let runtime_for_confirm = runtime_for_empty.clone();
            let secret_for_confirm = Arc::clone(&secret_for_empty);
            let vault_for_confirm = Arc::clone(&vault_for_empty);
            let master_for_confirm = master_for_empty.clone();
            let refresh_for_confirm = Rc::clone(&refresh_for_empty);
            let on_changed_for_confirm = Rc::clone(&on_changed_for_empty);
            Self::confirm_destructive(
                &window_for_empty,
                crate::tr!("trash-confirm-empty").as_str(),
                move || {
                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    let runtime_for_task = runtime_for_confirm.clone();
                    let secret_for_task = Arc::clone(&secret_for_confirm);
                    let vault_for_task = Arc::clone(&vault_for_confirm);
                    let master_for_task = master_for_confirm.clone();
                    std::thread::spawn(move || {
                        let result = runtime_for_task.block_on(async move {
                            let vault_id = Self::resolve_current_vault(
                                Arc::clone(&vault_for_task),
                                admin_user_id,
                                master_for_task,
                            )
                            .await?;
                            secret_for_task.empty_trash(vault_id).await?;
                            Ok::<(), crate::errors::AppError>(())
                        });
                        let _ = sender.send(result);
                    });

                    let refresh_for_result = Rc::clone(&refresh_for_confirm);
                    let on_changed_for_result = Rc::clone(&on_changed_for_confirm);
                    glib::MainContext::default().spawn_local(async move {
                        if matches!(receiver.await, Ok(Ok(()))) {
                            on_changed_for_result();
                            refresh_for_result();
                        }
                    });
                },
            );
        });

        refresh_trash();

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }

    async fn resolve_current_vault<TVault>(
        vault_service: Arc<TVault>,
        admin_user_id: Uuid,
        admin_master_key: Vec<u8>,
    ) -> Result<Uuid, crate::errors::AppError>
    where
        TVault: VaultService + Send + Sync + 'static,
    {
        let vaults = vault_service.list_user_vaults(admin_user_id).await?;
        let current = vaults
            .into_iter()
            .next()
            .ok_or_else(|| crate::errors::AppError::NotFound("vault not found".to_string()))?;

        let _vault_key = vault_service
            .open_vault(current.id, SecretBox::new(Box::new(admin_master_key)))
            .await?;
        Ok(current.id)
    }

    #[allow(clippy::too_many_arguments)]
    fn refresh_trash_list<TSecret, TVault>(
        application: adw::Application,
        parent_window: adw::ApplicationWindow,
        runtime_handle: Handle,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        admin_user_id: Uuid,
        admin_master_key: Vec<u8>,
        list: gtk4::ListBox,
        stack: gtk4::Stack,
        empty_title: gtk4::Label,
        empty_copy: gtk4::Label,
        on_changed: Rc<dyn Fn()>,
    ) where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        empty_title.set_text(crate::tr!("trash-loading-title").as_str());
        empty_copy.set_text(crate::tr!("trash-loading-copy").as_str());
        stack.set_visible_child_name("empty");

        let runtime_for_loader = runtime_handle.clone();
        let secret_for_loader = Arc::clone(&secret_service);

        let (sender, receiver) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let result: Result<Vec<TrashRowView>, crate::errors::AppError> = runtime_for_loader
                .block_on(async move {
                    let items = secret_for_loader
                        .list_all_trash_by_user(admin_user_id)
                        .await?;
                    let mut rows = Vec::with_capacity(items.len());
                    for item in items {
                        let login = item
                            .metadata_json
                            .as_deref()
                            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                            .and_then(|value| {
                                value
                                    .get("login")
                                    .and_then(Value::as_str)
                                    .map(ToString::to_string)
                            })
                            .unwrap_or_default();

                        rows.push(TrashRowView {
                            secret_id: item.id,
                            vault_id: item.vault_id,
                            title: item
                                .title
                                .unwrap_or_else(|| crate::tr!("trash-secret-fallback-title")),
                            type_label: match item.secret_type {
                                crate::models::SecretType::Password => {
                                    crate::tr!("secret-type-password")
                                }
                                crate::models::SecretType::ApiToken => {
                                    crate::tr!("secret-type-api-token")
                                }
                                crate::models::SecretType::SshKey => {
                                    crate::tr!("secret-type-ssh-key")
                                }
                                crate::models::SecretType::SecureDocument => {
                                    crate::tr!("secret-type-secure-document")
                                }
                            },
                            login,
                            deleted_at: item.deleted_at.clone(),
                        });
                    }
                    Ok(rows)
                });
            let _ = sender.send(result);
        });

        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(items)) => {
                    while let Some(child) = list.first_child() {
                        list.remove(&child);
                    }

                    if items.is_empty() {
                        empty_title.set_text(crate::tr!("trash-empty-title").as_str());
                        empty_copy.set_text(crate::tr!("trash-empty-description").as_str());
                        stack.set_visible_child_name("empty");
                        return;
                    }

                    for item in items {
                        let row = gtk4::ListBoxRow::new();
                        let row_box = gtk4::Box::builder()
                            .orientation(Orientation::Horizontal)
                            .spacing(10)
                            .margin_top(8)
                            .margin_bottom(8)
                            .margin_start(10)
                            .margin_end(10)
                            .build();

                        let text_box = gtk4::Box::builder()
                            .orientation(Orientation::Vertical)
                            .spacing(2)
                            .hexpand(true)
                            .build();

                        let title_label = gtk4::Label::new(Some(&item.title));
                        title_label.set_halign(Align::Start);

                        let meta = if item.login.is_empty() {
                            item.type_label.clone()
                        } else {
                            format!("{} • {}", item.type_label, item.login)
                        };
                        let meta_label = gtk4::Label::new(Some(&meta));
                        meta_label.set_halign(Align::Start);
                        meta_label.add_css_class("login-support-copy");

                        // Ligne de date de suppression
                        let deleted_text = item
                            .deleted_at
                            .as_deref()
                            .map(|raw| {
                                if raw.len() >= 16 {
                                    let date = &raw[..10];
                                    let time = &raw[11..16];
                                    crate::i18n::tr_args(
                                        "trash-deleted-at",
                                        &[
                                            ("date", crate::i18n::I18nArg::Str(date)),
                                            ("time", crate::i18n::I18nArg::Str(time)),
                                        ],
                                    )
                                } else {
                                    crate::i18n::tr_args(
                                        "trash-deleted-at-raw",
                                        &[("raw", crate::i18n::I18nArg::Str(raw))],
                                    )
                                }
                            })
                            .unwrap_or_default();
                        let deleted_label = gtk4::Label::new(Some(&deleted_text));
                        deleted_label.set_halign(Align::Start);
                        deleted_label.add_css_class("caption");
                        deleted_label.add_css_class("dim-label");

                        let actions = gtk4::Box::builder()
                            .orientation(Orientation::Horizontal)
                            .spacing(6)
                            .valign(Align::Center)
                            .build();

                        let restore_btn =
                            gtk4::Button::with_label(crate::tr!("trash-restore-button").as_str());
                        restore_btn.add_css_class("suggested-action");
                        let app_for_restore = application.clone();
                        let parent_for_restore = parent_window.clone();
                        let runtime_for_restore = runtime_handle.clone();
                        let secret_for_restore = Arc::clone(&secret_service);
                        let vault_for_restore = Arc::clone(&vault_service);
                        let list_for_restore = list.clone();
                        let stack_for_restore = stack.clone();
                        let empty_title_for_restore = empty_title.clone();
                        let empty_copy_for_restore = empty_copy.clone();
                        let master_for_restore = admin_master_key.clone();
                        let on_changed_for_restore = Rc::clone(&on_changed);
                        let restore_secret_id = item.secret_id;
                        let restore_vault_id = item.vault_id;
                        restore_btn.connect_clicked(move |_| {
                            let (sender, receiver) = tokio::sync::oneshot::channel();
                            let runtime_for_task = runtime_for_restore.clone();
                            let secret_for_task = Arc::clone(&secret_for_restore);
                            std::thread::spawn(move || {
                                let result = runtime_for_task.block_on(async move {
                                    secret_for_task
                                        .restore_secret(restore_secret_id, restore_vault_id)
                                        .await
                                });
                                let _ = sender.send(result);
                            });

                            let app_for_refresh = app_for_restore.clone();
                            let parent_for_refresh = parent_for_restore.clone();
                            let runtime_for_refresh = runtime_for_restore.clone();
                            let secret_for_refresh = Arc::clone(&secret_for_restore);
                            let vault_for_refresh = Arc::clone(&vault_for_restore);
                            let list_for_refresh = list_for_restore.clone();
                            let stack_for_refresh = stack_for_restore.clone();
                            let empty_title_refresh = empty_title_for_restore.clone();
                            let empty_copy_refresh = empty_copy_for_restore.clone();
                            let master_for_refresh = master_for_restore.clone();
                            let on_changed_refresh = Rc::clone(&on_changed_for_restore);
                            glib::MainContext::default().spawn_local(async move {
                                if matches!(receiver.await, Ok(Ok(()))) {
                                    on_changed_refresh();
                                    Self::refresh_trash_list(
                                        app_for_refresh.clone(),
                                        parent_for_refresh.clone(),
                                        runtime_for_refresh.clone(),
                                        Arc::clone(&secret_for_refresh),
                                        Arc::clone(&vault_for_refresh),
                                        admin_user_id,
                                        master_for_refresh.clone(),
                                        list_for_refresh.clone(),
                                        stack_for_refresh.clone(),
                                        empty_title_refresh.clone(),
                                        empty_copy_refresh.clone(),
                                        Rc::clone(&on_changed_refresh),
                                    );
                                }
                            });
                        });

                        let delete_btn =
                            gtk4::Button::with_label(crate::tr!("trash-delete-button").as_str());
                        delete_btn.add_css_class("destructive-action");
                        let app_for_delete = application.clone();
                        let parent_for_delete = parent_window.clone();
                        let runtime_for_delete = runtime_handle.clone();
                        let secret_for_delete = Arc::clone(&secret_service);
                        let vault_for_delete = Arc::clone(&vault_service);
                        let list_for_delete = list.clone();
                        let stack_for_delete = stack.clone();
                        let empty_title_for_delete = empty_title.clone();
                        let empty_copy_for_delete = empty_copy.clone();
                        let master_for_delete = admin_master_key.clone();
                        let on_changed_for_delete = Rc::clone(&on_changed);
                        let delete_secret_id = item.secret_id;
                        let delete_vault_id = item.vault_id;
                        let window_for_delete = parent_window.clone().upcast::<gtk4::Window>();
                        delete_btn.connect_clicked(move |_| {
                            let runtime_for_confirm = runtime_for_delete.clone();
                            let secret_for_confirm = Arc::clone(&secret_for_delete);
                            let app_for_confirm = app_for_delete.clone();
                            let parent_for_confirm = parent_for_delete.clone();
                            let vault_for_confirm = Arc::clone(&vault_for_delete);
                            let list_for_confirm = list_for_delete.clone();
                            let stack_for_confirm = stack_for_delete.clone();
                            let empty_title_for_confirm = empty_title_for_delete.clone();
                            let empty_copy_for_confirm = empty_copy_for_delete.clone();
                            let master_for_confirm = master_for_delete.clone();
                            let on_changed_for_confirm = Rc::clone(&on_changed_for_delete);
                            Self::confirm_destructive(
                                &window_for_delete,
                                crate::tr!("trash-confirm-delete").as_str(),
                                move || {
                                    let (sender, receiver) = tokio::sync::oneshot::channel();
                                    let runtime_for_task = runtime_for_confirm.clone();
                                    let secret_for_task = Arc::clone(&secret_for_confirm);
                                    std::thread::spawn(move || {
                                        let result = runtime_for_task.block_on(async move {
                                            secret_for_task
                                                .permanent_delete(delete_secret_id, delete_vault_id)
                                                .await
                                        });
                                        let _ = sender.send(result);
                                    });

                                    let app_for_refresh = app_for_confirm.clone();
                                    let parent_for_refresh = parent_for_confirm.clone();
                                    let runtime_for_refresh = runtime_for_confirm.clone();
                                    let secret_for_refresh = Arc::clone(&secret_for_confirm);
                                    let vault_for_refresh = Arc::clone(&vault_for_confirm);
                                    let list_for_refresh = list_for_confirm.clone();
                                    let stack_for_refresh = stack_for_confirm.clone();
                                    let empty_title_refresh = empty_title_for_confirm.clone();
                                    let empty_copy_refresh = empty_copy_for_confirm.clone();
                                    let master_for_refresh = master_for_confirm.clone();
                                    let on_changed_refresh = Rc::clone(&on_changed_for_confirm);
                                    glib::MainContext::default().spawn_local(async move {
                                        if matches!(receiver.await, Ok(Ok(()))) {
                                            on_changed_refresh();
                                            Self::refresh_trash_list(
                                                app_for_refresh.clone(),
                                                parent_for_refresh.clone(),
                                                runtime_for_refresh.clone(),
                                                Arc::clone(&secret_for_refresh),
                                                Arc::clone(&vault_for_refresh),
                                                admin_user_id,
                                                master_for_refresh.clone(),
                                                list_for_refresh.clone(),
                                                stack_for_refresh.clone(),
                                                empty_title_refresh.clone(),
                                                empty_copy_refresh.clone(),
                                                Rc::clone(&on_changed_refresh),
                                            );
                                        }
                                    });
                                },
                            );
                        });

                        text_box.append(&title_label);
                        text_box.append(&meta_label);
                        if !deleted_text.is_empty() {
                            text_box.append(&deleted_label);
                        }
                        actions.append(&restore_btn);
                        actions.append(&delete_btn);

                        row_box.append(&text_box);
                        row_box.append(&actions);
                        row.set_child(Some(&row_box));
                        list.append(&row);
                    }

                    stack.set_visible_child_name("list");
                }
                Ok(Err(_)) | Err(_) => {
                    empty_title.set_text(crate::tr!("trash-unavailable-title").as_str());
                    empty_copy.set_text(crate::tr!("trash-unavailable-copy").as_str());
                    stack.set_visible_child_name("empty");
                }
            }
        });
    }

    fn confirm_destructive(parent: &gtk4::Window, message: &str, on_confirm: impl Fn() + 'static) {
        let dialog = gtk4::MessageDialog::builder()
            .transient_for(parent)
            .modal(true)
            .text(crate::tr!("trash-dialog-title").as_str())
            .secondary_text(message)
            .build();
        dialog.add_button(
            crate::tr!("trash-dialog-cancel").as_str(),
            gtk4::ResponseType::Cancel,
        );
        dialog.add_button(
            crate::tr!("trash-dialog-confirm").as_str(),
            gtk4::ResponseType::Accept,
        );
        dialog.connect_response(move |d, response| {
            if response == gtk4::ResponseType::Accept {
                on_confirm();
            }
            d.close();
        });
        dialog.present();
    }
}

struct TrashRowView {
    secret_id: Uuid,
    vault_id: Uuid,
    title: String,
    type_label: String,
    login: String,
    deleted_at: Option<String>,
}

use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use secrecy::{ExposeSecret, SecretBox};
use serde_json::{Map, Value};
// `time` est utilise ici (et non `chrono`) car ce calcul est purement UTC —
// OffsetDateTime::now_utc() n'appelle pas localtime_r et est safe en multi-thread.
// Ne pas migrer vers chrono::Utc sans verifier que Duration::days est disponible
// avec la meme semantique.
use time::format_description::well_known::Rfc3339;
use time::Duration;
use time::OffsetDateTime;
use tokio::runtime::Handle;
use tracing::warn;
use uuid::Uuid;

use crate::models::SecretType;
use crate::services::password_service::{PasswordService, PasswordServiceImpl};
use crate::services::secret_service::SecretService;
use crate::services::vault_service::VaultService;
use crate::ui::widgets::password_strength_bar::PasswordStrengthBar;

#[derive(Clone, Copy, Debug)]
pub enum DialogMode {
    Create,
    CreateInVault(Uuid),
    Edit(Uuid),
}

pub struct AddEditDialog;

pub struct AddEditInlineView {
    pub container: gtk4::ScrolledWindow,
}

impl AddEditDialog {
    fn stack_name_for_secret_type(secret_type: SecretType) -> &'static str {
        match secret_type {
            SecretType::Password => "password",
            SecretType::ApiToken => "api_token",
            SecretType::SshKey => "ssh_key",
            SecretType::SecureDocument => "secure_document",
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_inline<TSecret, TVault>(
        runtime_handle: Handle,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        admin_user_id: Uuid,
        admin_master_key: Vec<u8>,
        show_passwords_in_edit: bool,
        mode: DialogMode,
        on_cancel: impl Fn() + 'static,
        on_saved: impl Fn(String) + 'static,
    ) -> AddEditInlineView
    where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        include!("add_edit_dialog/parts/build_inline_body.inc")
    }

    fn build_labeled_entry(
        label_text: &str,
        placeholder: &str,
        css_class: &str,
    ) -> (gtk4::Box, gtk4::Entry) {
        let box_widget = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .build();

        let label = gtk4::Label::new(Some(label_text));
        label.add_css_class("login-field-label");
        label.set_halign(Align::Start);

        let entry = gtk4::Entry::builder().placeholder_text(placeholder).build();
        entry.add_css_class("login-entry");
        entry.add_css_class(css_class);

        box_widget.append(&label);
        box_widget.append(&entry);
        (box_widget, entry)
    }

    fn build_password_panel(
        edit_hint: Option<&str>,
    ) -> (gtk4::Frame, gtk4::PasswordEntry, PasswordStrengthBar) {
        let frame = gtk4::Frame::new(None);
        frame.add_css_class("dialog-type-frame");

        let box_widget = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let password_label = gtk4::Label::new(Some(crate::tr!("add-edit-password-label").as_str()));
        password_label.add_css_class("login-field-label");
        password_label.set_halign(Align::Start);

        let password_row = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();

        let password_entry = gtk4::PasswordEntry::builder()
            .placeholder_text(crate::tr!("add-edit-password-placeholder").as_str())
            .show_peek_icon(true)
            .hexpand(true)
            .build();
        password_entry.add_css_class("login-entry");

        let generate_button =
            gtk4::Button::with_label(crate::tr!("add-edit-password-generate").as_str());
        generate_button.add_css_class("secondary-pill");

        let generated_password_entry = password_entry.clone();
        generate_button.connect_clicked(move |_| {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let service = PasswordServiceImpl::new();
                let result = service.generate_password(24);
                let _ = sender.send(result);
            });

            let entry_for_result = generated_password_entry.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Ok(Ok(value)) = receiver.await {
                    if let Ok(text) = String::from_utf8(value.expose_secret().clone()) {
                        entry_for_result.set_text(&text);
                    }
                }
            });
        });

        password_row.append(&password_entry);
        password_row.append(&generate_button);

        let strength_bar = PasswordStrengthBar::new();
        strength_bar.connect_to_password_entry(&password_entry);

        box_widget.append(&password_label);
        box_widget.append(&password_row);
        if let Some(hint) = edit_hint {
            let hint_wrap = gtk4::Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(8)
                .margin_top(4)
                .margin_bottom(2)
                .margin_start(4)
                .margin_end(4)
                .build();
            hint_wrap.add_css_class("dialog-password-edit-hint-wrap");

            let hint_icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
            hint_icon.set_pixel_size(16);
            hint_icon.add_css_class("dialog-password-edit-hint-icon");

            let hint_label = gtk4::Label::new(Some(hint));
            hint_label.set_halign(Align::Start);
            hint_label.set_xalign(0.0);
            hint_label.set_wrap(true);
            hint_label.add_css_class("dialog-password-edit-hint");
            hint_label.set_hexpand(true);

            hint_wrap.append(&hint_icon);
            hint_wrap.append(&hint_label);
            box_widget.append(&hint_wrap);
        }
        box_widget.append(strength_bar.root());
        frame.set_child(Some(&box_widget));
        (frame, password_entry, strength_bar)
    }

    fn build_api_token_panel() -> (gtk4::Frame, gtk4::PasswordEntry, gtk4::Entry) {
        let frame = gtk4::Frame::new(None);
        frame.add_css_class("dialog-type-frame");

        let box_widget = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let token_label = gtk4::Label::new(Some(crate::tr!("add-edit-api-token-label").as_str()));
        token_label.add_css_class("login-field-label");
        token_label.set_halign(Align::Start);

        let token_entry = gtk4::PasswordEntry::builder()
            .placeholder_text(crate::tr!("add-edit-api-token-placeholder").as_str())
            .show_peek_icon(true)
            .build();
        token_entry.add_css_class("login-entry");

        let (provider_row, provider_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-api-provider-label").as_str(),
            crate::tr!("add-edit-api-provider-placeholder").as_str(),
            "dialog-api-provider-entry",
        );

        box_widget.append(&token_label);
        box_widget.append(&token_entry);
        box_widget.append(&provider_row);
        frame.set_child(Some(&box_widget));
        (frame, token_entry, provider_entry)
    }

    fn build_ssh_key_panel() -> (gtk4::Frame, gtk4::TextView, gtk4::Entry, gtk4::Entry) {
        let frame = gtk4::Frame::new(None);
        frame.add_css_class("dialog-type-frame");

        let box_widget = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let private_key_label =
            gtk4::Label::new(Some(crate::tr!("add-edit-ssh-private-label").as_str()));
        private_key_label.add_css_class("login-field-label");
        private_key_label.set_halign(Align::Start);

        let private_key_scrolled = gtk4::ScrolledWindow::builder()
            .min_content_height(120)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();

        let private_key_text = gtk4::TextView::new();
        private_key_text.set_wrap_mode(gtk4::WrapMode::WordChar);
        private_key_text.add_css_class("dialog-ssh-private-key-text");
        private_key_scrolled.set_child(Some(&private_key_text));

        let (public_row, public_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-ssh-public-label").as_str(),
            crate::tr!("add-edit-ssh-public-placeholder").as_str(),
            "dialog-ssh-public-entry",
        );
        let (passphrase_row, passphrase_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-ssh-passphrase-label").as_str(),
            crate::tr!("add-edit-ssh-passphrase-placeholder").as_str(),
            "dialog-ssh-passphrase-entry",
        );

        box_widget.append(&private_key_label);
        box_widget.append(&private_key_scrolled);
        box_widget.append(&public_row);
        box_widget.append(&passphrase_row);
        frame.set_child(Some(&box_widget));
        (frame, private_key_text, public_entry, passphrase_entry)
    }

    fn build_secure_document_panel() -> (gtk4::Frame, gtk4::Entry, gtk4::Entry) {
        let frame = gtk4::Frame::new(None);
        frame.add_css_class("dialog-type-frame");

        let box_widget = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        let (path_row, path_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-document-path-label").as_str(),
            crate::tr!("add-edit-document-path-placeholder").as_str(),
            "dialog-document-path-entry",
        );
        let (mime_row, mime_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-document-mime-label").as_str(),
            crate::tr!("add-edit-document-mime-placeholder").as_str(),
            "dialog-document-mime-entry",
        );

        let import_hint =
            gtk4::Label::new(Some(crate::tr!("add-edit-document-import-hint").as_str()));
        import_hint.add_css_class("login-support-copy");
        import_hint.set_wrap(true);
        import_hint.set_halign(Align::Start);

        box_widget.append(&path_row);
        box_widget.append(&mime_row);
        box_widget.append(&import_hint);
        frame.set_child(Some(&box_widget));
        (frame, path_entry, mime_entry)
    }

    #[allow(clippy::too_many_arguments)]
    fn setup_for_edit<TSecret, TVault>(
        runtime_handle: Handle,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        admin_user_id: Uuid,
        admin_master_key: Vec<u8>,
        show_passwords_in_edit: bool,
        secret_id: Uuid,
        title_entry: gtk4::Entry,
        category_entry: gtk4::Entry,
        tags_entry: gtk4::Entry,
        type_dropdown: gtk4::DropDown,
        password_entry: gtk4::PasswordEntry,
        initial_password_snapshot: Rc<std::cell::RefCell<Option<String>>>,
        username_entry: gtk4::Entry,
        url_entry: gtk4::Entry,
        notes_buffer: gtk4::TextBuffer,
        validity_unlimited: gtk4::CheckButton,
        validity_days: gtk4::SpinButton,
        api_provider_entry: gtk4::Entry,
        ssh_public_entry: gtk4::Entry,
        ssh_passphrase_entry: gtk4::Entry,
        secure_doc_mime_entry: gtk4::Entry,
        error_label: gtk4::Label,
    ) where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let result: Result<
                (crate::models::SecretItem, Option<String>),
                crate::errors::AppError,
            > = runtime_handle.block_on(async move {
                let vaults = vault_service.list_user_vaults(admin_user_id).await?;
                let mut found: Option<(Uuid, crate::models::SecretItem)> = None;
                for vault in vaults {
                    let items = secret_service.list_by_vault(vault.id).await?;
                    if let Some(item) = items.into_iter().find(|item| item.id == secret_id) {
                        found = Some((vault.id, item));
                        break;
                    }
                }
                let (target_vault_id, item) = found.ok_or_else(|| {
                    crate::errors::AppError::NotFound("secret not found".to_string())
                })?;

                let existing_password =
                    if show_passwords_in_edit && matches!(item.secret_type, SecretType::Password) {
                        let vault_key = vault_service
                            .open_vault_for_user(
                                admin_user_id,
                                target_vault_id,
                                SecretBox::new(Box::new(admin_master_key.clone())),
                            )
                            .await?;
                        let decrypted = secret_service
                            .get_secret(
                                secret_id,
                                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
                            )
                            .await?;
                        Some(
                            String::from_utf8(decrypted.secret_value.expose_secret().clone())
                                .unwrap_or_default(),
                        )
                    } else {
                        None
                    };

                Ok((item, existing_password))
            });
            let _ = sender.send(result);
        });

        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok((item, existing_password))) => {
                    title_entry.set_text(item.title.as_deref().unwrap_or_default());
                    tags_entry.set_text(item.tags.as_deref().unwrap_or_default());

                    let type_index = match item.secret_type {
                        SecretType::Password => SecretType::Password.dropdown_index(),
                        SecretType::ApiToken => SecretType::ApiToken.dropdown_index(),
                        SecretType::SshKey => SecretType::SshKey.dropdown_index(),
                        SecretType::SecureDocument => SecretType::SecureDocument.dropdown_index(),
                    };
                    type_dropdown.set_selected(type_index);

                    if let Some(raw_metadata) = item.metadata_json {
                        if let Ok(value) = serde_json::from_str::<Value>(&raw_metadata) {
                            category_entry.set_text(
                                value
                                    .get("category")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default(),
                            );
                            username_entry.set_text(
                                value
                                    .get("login")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default(),
                            );
                            url_entry.set_text(
                                value.get("url").and_then(Value::as_str).unwrap_or_default(),
                            );
                            notes_buffer.set_text(
                                value
                                    .get("notes")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default(),
                            );

                            api_provider_entry.set_text(
                                value
                                    .get("provider")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default(),
                            );
                            ssh_public_entry.set_text(
                                value
                                    .get("ssh_public_key")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default(),
                            );
                            ssh_passphrase_entry.set_text(
                                value
                                    .get("ssh_passphrase")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default(),
                            );
                            secure_doc_mime_entry.set_text(
                                value
                                    .get("document_mime")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default(),
                            );

                            let unlimited = value
                                .get("validity_unlimited")
                                .and_then(Value::as_bool)
                                .unwrap_or(item.expires_at.is_none());
                            validity_unlimited.set_active(unlimited);

                            if let Some(days) = value.get("validity_days").and_then(Value::as_i64) {
                                if days > 0 {
                                    validity_days.set_value(days as f64);
                                }
                            }
                        }
                    } else {
                        validity_unlimited.set_active(item.expires_at.is_none());
                    }

                    if let Some(value) = existing_password {
                        *initial_password_snapshot.borrow_mut() = Some(value.clone());
                        password_entry.set_text(value.as_str());
                    }

                    validity_days.set_sensitive(!validity_unlimited.is_active());
                }
                Ok(Err(_)) | Err(_) => {
                    error_label.set_text(crate::tr!("add-edit-error-load-failed").as_str());
                    error_label.set_visible(true);
                }
            }
        });
    }
}

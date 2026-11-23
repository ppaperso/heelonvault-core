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
use time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::runtime::Handle;
use tracing::{error, warn};
use uuid::Uuid;

use crate::ui::widgets::password_generator::{self, PasswordGenerator};
use crate::ui::widgets::password_strength_bar::PasswordStrengthBar;
use heelonvault_core::models::{PasswordKind, SecretType};
use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

mod lifecycle;
mod operation;
#[cfg(test)]
mod tests;
pub(crate) use lifecycle::EditorState;
#[cfg(test)]
pub(crate) use lifecycle::native_lifecycle_scenario;
#[cfg(test)]
pub(crate) use tests::native_inline_scenario;

#[cfg(test)]
pub(crate) use operation::native_operation_scenario;

#[cfg(test)]
pub(crate) fn native_password_hint_scenario() {
    fn has_hint(widget: &gtk4::Widget) -> bool {
        if widget.has_css_class("dialog-password-edit-hint-wrap") {
            return true;
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            if has_hint(&current) {
                return true;
            }
            child = current.next_sibling();
        }
        false
    }

    for mode in [
        DialogMode::Create,
        DialogMode::CreateInVault(Uuid::nil()),
        DialogMode::Edit(Uuid::nil()),
    ] {
        for show_passwords in [false, true] {
            let panel = AddEditDialog::build_password_panel(mode, show_passwords);
            let entry = panel.entry.clone();
            assert_eq!(
                has_hint(panel.frame.upcast_ref()),
                matches!(mode, DialogMode::Edit(_)) && !show_passwords,
                "{mode:?}, show_passwords_in_edit={show_passwords}"
            );
            assert!(entry.text().is_empty());
        }
    }
}

/// Password panel of the editor: value, kind selector, generator and strength bar.
#[derive(Clone)]
pub(crate) struct PasswordPanel {
    pub(crate) frame: gtk4::Frame,
    pub(crate) entry: gtk4::PasswordEntry,
    pub(crate) strength_bar: PasswordStrengthBar,
    kind_password: gtk4::ToggleButton,
    kind_passphrase: gtk4::ToggleButton,
    generate_button: gtk4::ToggleButton,
    pub(crate) generator: PasswordGenerator,
}

impl PasswordPanel {
    pub(crate) fn kind(&self) -> PasswordKind {
        if self.kind_passphrase.is_active() {
            PasswordKind::Passphrase
        } else {
            PasswordKind::Password
        }
    }

    pub(crate) fn set_kind(&self, kind: PasswordKind) {
        match kind {
            PasswordKind::Passphrase => self.kind_passphrase.set_active(true),
            _ => self.kind_password.set_active(true),
        }
    }

    /// Kind and generator recipe to store in the item metadata. The recipe is kept only when
    /// the value comes from the generator (`applied`) or is unchanged (`loaded`).
    pub(crate) fn metadata_entries(
        &self,
        password_unchanged: bool,
        loaded_recipe: Option<&Value>,
    ) -> Vec<(String, Value)> {
        let mut entries = vec![(
            PasswordKind::METADATA_KEY.to_string(),
            Value::String(self.kind().as_metadata_value().to_string()),
        )];
        let recipe = match self.generator.applied_recipe() {
            Some(recipe) => serde_json::to_value(recipe).ok(),
            None if password_unchanged => loaded_recipe.cloned(),
            None => None,
        };
        if let Some(recipe) = recipe {
            entries.push((PasswordKind::RECIPE_METADATA_KEY.to_string(), recipe));
        }
        entries
    }

    pub(crate) fn refresh_i18n(&self) {
        self.kind_password
            .set_label(&heelonvault_core::tr!("password-kind-password"));
        self.kind_passphrase
            .set_label(&heelonvault_core::tr!("password-kind-passphrase"));
        self.kind_passphrase
            .set_tooltip_text(Some(&heelonvault_core::tr!(
                "password-kind-passphrase-tooltip"
            )));
        self.generate_button
            .set_label(&heelonvault_core::tr!("add-edit-password-generate"));
        self.generator.refresh_i18n();
        self.strength_bar.refresh_i18n();
    }
}

/// Kind selector, metadata entries and fingerprint exclusion of the password panel.
#[cfg(test)]
pub(crate) fn native_password_panel_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let panel = AddEditDialog::build_password_panel(DialogMode::Create, false);
    let root = gtk4::Box::new(Orientation::Vertical, 0);
    root.append(&panel.frame);
    let dirty_label = gtk4::Label::new(None);
    let state = EditorState::new(&root, &dirty_label);
    state.finish_loading();

    // Generator settings are not part of the secret: the editor stays clean.
    panel.generator.settings_widget().set_active(false);
    panel.generator.settings_widget().set_active(true);
    panel.generate_button.set_active(true);
    assert!(panel.generator.is_open());
    assert!(
        !state.dirty(),
        "generator settings must not mark the editor dirty"
    );

    // The kind is part of the item.
    panel.set_kind(PasswordKind::Passphrase);
    assert!(state.dirty());
    assert_eq!(panel.kind(), PasswordKind::Passphrase);
    assert!(panel.strength_bar.root().first_child().is_some());

    panel.generator.click_use();
    assert!(
        !panel.generate_button.is_active(),
        "toggle follows the closed panel"
    );
    let entries = panel.metadata_entries(false, None);
    let lookup = |key: &str| {
        entries
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value.clone())
    };
    assert_eq!(
        lookup(PasswordKind::METADATA_KEY),
        Some(Value::String("passphrase".to_string()))
    );
    let recipe = lookup(PasswordKind::RECIPE_METADATA_KEY).ok_or("missing recipe")?;
    assert_eq!(
        recipe.get("kind").and_then(Value::as_str),
        Some("passphrase")
    );

    // Edited by hand: no recipe, unless the stored value is unchanged.
    panel.entry.set_text("Manual-Passphrase-Value-2026");
    assert_eq!(panel.metadata_entries(false, None).len(), 1);
    let loaded = Value::String("loaded".to_string());
    assert!(panel.metadata_entries(true, Some(&loaded)).contains(&(
        PasswordKind::RECIPE_METADATA_KEY.to_string(),
        loaded.clone()
    )));
    Ok(())
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum DialogMode {
    Create,
    CreateInVault(Uuid),
    Edit(Uuid),
}

#[allow(dead_code)]
pub struct AddEditDialog;

#[allow(dead_code)]
pub struct AddEditInlineView {
    pub container: gtk4::ScrolledWindow,
    pub(crate) state: EditorState,
    /// Category/tags row; receives suggestions from the loaded secrets.
    pub(crate) classification: crate::ui::widgets::classification::ClassificationField,
}

#[allow(dead_code)]
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
        operation_busy: Rc<std::cell::Cell<bool>>,
        critical_operations: Rc<std::cell::Cell<u32>>,
        on_cancel: impl Fn() + 'static,
        on_saved: impl Fn(String) + 'static,
        on_deleted: impl Fn(String) + 'static,
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

    fn build_password_panel(mode: DialogMode, show_passwords_in_edit: bool) -> PasswordPanel {
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

        let password_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("add-edit-password-label").as_str(),
        ));
        password_label.add_css_class("login-field-label");
        password_label.set_halign(Align::Start);
        password_label.set_hexpand(true);

        // Password | Passphrase: what the stored value is (saved in the metadata) and which
        // generator the panel shows.
        let kind_password = gtk4::ToggleButton::new();
        let kind_passphrase = gtk4::ToggleButton::new();
        for button in [&kind_password, &kind_passphrase] {
            button.add_css_class("add-edit-type-pill");
            button.add_css_class("password-kind-pill");
        }
        kind_passphrase.set_group(Some(&kind_password));
        kind_password.set_active(true);
        let kind_row = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .halign(Align::End)
            .spacing(6)
            .build();
        kind_row.add_css_class("password-kind-selector");
        kind_row.append(&kind_password);
        kind_row.append(&kind_passphrase);

        let label_row = gtk4::Box::new(Orientation::Horizontal, 8);
        label_row.append(&password_label);
        label_row.append(&kind_row);

        let password_row = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();

        let password_entry = gtk4::PasswordEntry::builder()
            .placeholder_text(heelonvault_core::tr!("add-edit-password-placeholder").as_str())
            .show_peek_icon(true)
            .hexpand(true)
            .build();
        password_entry.add_css_class("login-entry");

        let generator = PasswordGenerator::new(&password_entry);
        let generate_button = gtk4::ToggleButton::new();
        generate_button.add_css_class("secondary-pill");
        generate_button.add_css_class("password-generator-toggle");
        generate_button.add_css_class(password_generator::FINGERPRINT_SKIP_CLASS);
        {
            let generator = generator.clone();
            generate_button.connect_toggled(move |button| {
                generator.set_open(button.is_active());
            });
        }
        {
            // The panel closes itself after use: keep the toggle in sync.
            let button = generate_button.downgrade();
            generator
                .widget()
                .connect_reveal_child_notify(move |revealer| {
                    if let Some(button) = button.upgrade()
                        && button.is_active() != revealer.reveals_child()
                    {
                        button.set_active(revealer.reveals_child());
                    }
                });
        }

        password_row.append(&password_entry);
        password_row.append(&generate_button);

        let strength_bar = PasswordStrengthBar::for_stored_passwords();
        strength_bar.connect_to_password_entry(&password_entry);

        for (button, kind) in [
            (&kind_password, PasswordKind::Password),
            (&kind_passphrase, PasswordKind::Passphrase),
        ] {
            let generator = generator.clone();
            let strength_bar = strength_bar.clone();
            button.connect_toggled(move |button| {
                if button.is_active() {
                    generator.set_kind(kind);
                    strength_bar.set_passphrase_mode(kind == PasswordKind::Passphrase);
                }
            });
        }

        box_widget.append(&label_row);
        box_widget.append(&password_row);
        box_widget.append(generator.widget());
        if matches!(mode, DialogMode::Edit(_)) && !show_passwords_in_edit {
            let hint_wrap = gtk4::Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(8)
                .margin_top(4)
                .margin_bottom(2)
                .margin_start(4)
                .margin_end(4)
                .build();
            hint_wrap.add_css_class("dialog-password-edit-hint-wrap");

            let hint_icon = gtk4::Image::from_icon_name("hv-info-symbolic");
            hint_icon.set_pixel_size(16);
            hint_icon.add_css_class("dialog-password-edit-hint-icon");

            let hint_label = gtk4::Label::new(Some(
                heelonvault_core::tr!("add-edit-password-edit-hint").as_str(),
            ));
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
        let panel = PasswordPanel {
            frame,
            entry: password_entry,
            strength_bar,
            kind_password,
            kind_passphrase,
            generate_button,
            generator,
        };
        panel.refresh_i18n();
        panel
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

        let token_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("add-edit-api-token-label").as_str(),
        ));
        token_label.add_css_class("login-field-label");
        token_label.set_halign(Align::Start);

        let token_entry = gtk4::PasswordEntry::builder()
            .placeholder_text(heelonvault_core::tr!("add-edit-api-token-placeholder").as_str())
            .show_peek_icon(true)
            .build();
        token_entry.add_css_class("login-entry");

        let (provider_row, provider_entry) = Self::build_labeled_entry(
            heelonvault_core::tr!("add-edit-api-provider-label").as_str(),
            heelonvault_core::tr!("add-edit-api-provider-placeholder").as_str(),
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

        let private_key_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("add-edit-ssh-private-label").as_str(),
        ));
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
            heelonvault_core::tr!("add-edit-ssh-public-label").as_str(),
            heelonvault_core::tr!("add-edit-ssh-public-placeholder").as_str(),
            "dialog-ssh-public-entry",
        );
        let (passphrase_row, passphrase_entry) = Self::build_labeled_entry(
            heelonvault_core::tr!("add-edit-ssh-passphrase-label").as_str(),
            heelonvault_core::tr!("add-edit-ssh-passphrase-placeholder").as_str(),
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
            heelonvault_core::tr!("add-edit-document-path-label").as_str(),
            heelonvault_core::tr!("add-edit-document-path-placeholder").as_str(),
            "dialog-document-path-entry",
        );
        let (mime_row, mime_entry) = Self::build_labeled_entry(
            heelonvault_core::tr!("add-edit-document-mime-label").as_str(),
            heelonvault_core::tr!("add-edit-document-mime-placeholder").as_str(),
            "dialog-document-mime-entry",
        );

        let import_hint = gtk4::Label::new(Some(
            heelonvault_core::tr!("add-edit-document-import-hint").as_str(),
        ));
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
        password_panel: PasswordPanel,
        loaded_recipe: Rc<std::cell::RefCell<Option<Value>>>,
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
        health_access_check: gtk4::CheckButton,
        error_label: gtk4::Label,
        source_vault_id_state: Rc<std::cell::RefCell<Option<Uuid>>>,
        target_vault_id_state: Rc<std::cell::RefCell<Option<Uuid>>>,
        vault_choices_state: Rc<std::cell::RefCell<Vec<(Uuid, String)>>>,
        vaults_label: gtk4::Label,
        vault_pills_row: gtk4::Box,
        lifecycle: EditorState,
        on_loaded: Rc<dyn Fn()>,
    ) where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        let password_entry = password_panel.entry.clone();
        type EditLoadResult = Result<
            (
                heelonvault_core::models::SecretItem,
                Option<zeroize::Zeroizing<String>>,
                Uuid,
                Vec<(Uuid, String)>,
            ),
            heelonvault_core::errors::AppError,
        >;

        // Clones kept outside the first thread for the SecretViewed audit call below.
        let admin_master_key = zeroize::Zeroizing::new(admin_master_key);
        let secret_for_audit = Arc::clone(&secret_service);
        let runtime_for_audit = runtime_handle.clone();

        let (sender, receiver) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let result: EditLoadResult = runtime_handle.block_on(async move {
                let vaults = vault_service.list_user_vaults(admin_user_id).await?;
                let vault_choices = vaults
                    .iter()
                    .map(|vault| (vault.id, vault.name.clone()))
                    .collect::<Vec<(Uuid, String)>>();
                let mut found: Option<(Uuid, heelonvault_core::models::SecretItem)> = None;
                for vault in vaults {
                    let items = secret_service.list_by_vault(vault.id).await?;
                    if let Some(item) = items.into_iter().find(|item| item.id == secret_id) {
                        found = Some((vault.id, item));
                        break;
                    }
                }
                let (target_vault_id, item) = found.ok_or_else(|| {
                    heelonvault_core::errors::AppError::NotFound("secret not found".to_string())
                })?;
                let existing_password =
                    if show_passwords_in_edit && matches!(item.secret_type, SecretType::Password) {
                        let vault_key = vault_service
                            .open_vault_for_user(
                                admin_user_id,
                                target_vault_id,
                                SecretBox::new(Box::new(admin_master_key.to_vec())),
                            )
                            .await?;
                        let decrypted = secret_service
                            .get_secret(
                                secret_id,
                                SecretBox::new(Box::new(vault_key.expose_secret().clone())),
                            )
                            .await?;
                        Some(zeroize::Zeroizing::new(
                            String::from_utf8(decrypted.secret_value.expose_secret().clone())
                                .unwrap_or_default(),
                        ))
                    } else {
                        None
                    };

                Ok((item, existing_password, target_vault_id, vault_choices))
            });
            let _ = sender.send(result);
        });

        glib::MainContext::default().spawn_local(async move {
            let result = receiver.await;
            if !lifecycle.active() {
                return;
            }
            match result {
                Ok(Ok((item, existing_password, source_vault_id, vault_choices))) => {
                    *source_vault_id_state.borrow_mut() = Some(source_vault_id);
                    *target_vault_id_state.borrow_mut() = Some(source_vault_id);
                    *vault_choices_state.borrow_mut() = vault_choices;

                    while let Some(child) = vault_pills_row.first_child() {
                        vault_pills_row.remove(&child);
                    }

                    let mut group_leader: Option<gtk4::CheckButton> = None;
                    for (vault_id, vault_name) in vault_choices_state.borrow().iter() {
                        let pill = gtk4::CheckButton::with_label(vault_name.as_str());
                        pill.add_css_class("add-edit-vault-pill");

                        if let Some(ref leader) = group_leader {
                            pill.set_group(Some(leader));
                        } else {
                            group_leader = Some(pill.clone());
                        }

                        let target_for_toggle = Rc::clone(&target_vault_id_state);
                        let id_for_toggle = *vault_id;
                        pill.connect_toggled(move |btn| {
                            if btn.is_active() {
                                *target_for_toggle.borrow_mut() = Some(id_for_toggle);
                            }
                        });

                        if *vault_id == source_vault_id {
                            pill.set_active(true);
                        }

                        vault_pills_row.append(&pill);
                    }

                    vaults_label.set_visible(true);
                    vault_pills_row.set_visible(true);

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

                            if matches!(item.secret_type, SecretType::Password) {
                                password_panel.set_kind(PasswordKind::from_metadata(&value));
                                if let Some(recipe) = value.get(PasswordKind::RECIPE_METADATA_KEY)
                                {
                                    if let Ok(parsed) = serde_json::from_value::<
                                        heelonvault_core::services::password_generator::GeneratorRecipe,
                                    >(recipe.clone())
                                    {
                                        password_panel.generator.set_recipe(&parsed);
                                    }
                                    *loaded_recipe.borrow_mut() = Some(recipe.clone());
                                }
                            }

                            let unlimited = value
                                .get("validity_unlimited")
                                .and_then(Value::as_bool)
                                .unwrap_or(item.expires_at.is_none());
                            validity_unlimited.set_active(unlimited);
                            health_access_check.set_active(
                                value
                                    .get("health_access")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                            );

                            if let Some(days) = value.get("validity_days").and_then(Value::as_i64)
                                && days > 0
                            {
                                validity_days.set_value(days as f64);
                            }
                        }
                    } else {
                        validity_unlimited.set_active(item.expires_at.is_none());
                    }

                    if let Some(value) = existing_password {
                        *initial_password_snapshot.borrow_mut() = Some(value.to_string());
                        password_entry.set_text(value.as_str());
                    }

                    validity_days.set_sensitive(!validity_unlimited.is_active());
                    on_loaded();

                    // ── CNIL: log SecretViewed ──────────────────────────────
                    let title_for_audit = item.title.clone();
                    std::thread::spawn(move || {
                        let _ = runtime_for_audit.block_on(async move {
                            secret_for_audit
                                .record_viewed(
                                    secret_id,
                                    Some(admin_user_id),
                                    title_for_audit.as_deref(),
                                )
                                .await
                        });
                    });
                }
                Ok(Err(error)) => {
                    error!(%error, "secret editor load failed");
                    error_label
                        .set_text(heelonvault_core::tr!("add-edit-error-load-failed").as_str());
                    error_label.set_visible(true);
                }
                Err(error) => {
                    error!(%error, "secret editor load task lost");
                    error_label
                        .set_text(heelonvault_core::tr!("add-edit-error-load-failed").as_str());
                    error_label.set_visible(true);
                }
            }
        });
    }
}

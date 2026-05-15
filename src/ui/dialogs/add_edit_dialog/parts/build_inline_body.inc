{
        let on_saved: Rc<dyn Fn(String)> = Rc::new(on_saved);
        let on_cancel: Rc<dyn Fn()> = Rc::new(on_cancel);

        let container = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();

        let root = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        root.add_css_class("add-edit-dialog");

        let header_card = gtk4::Frame::new(None);
        header_card.add_css_class("login-hero");

        let header_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(14)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        let back_button = gtk4::Button::with_label(crate::tr!("add-edit-button-back").as_str());
        back_button.add_css_class("flat");
        let on_cancel_for_back = Rc::clone(&on_cancel);
        back_button.connect_clicked(move |_| {
            on_cancel_for_back();
        });

        let header_icon = gtk4::Image::from_resource(
            "/com/heelonvault/rust/icons/hicolor/128x128/apps/heelonvault.png",
        );
        header_icon.set_pixel_size(42);
        header_icon.add_css_class("login-hero-icon");

        let header_text = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .build();

        let inline_header_title_text = match mode {
            DialogMode::Create | DialogMode::CreateInVault(_) => {
                crate::tr!("add-edit-header-title-create")
            }
            DialogMode::Edit(_) => crate::tr!("add-edit-header-title-edit"),
        };
        let title = gtk4::Label::new(Some(inline_header_title_text.as_str()));
        title.add_css_class("title-2");
        title.add_css_class("login-hero-title");
        title.set_halign(Align::Start);

        let inline_subtitle_text = match mode {
            DialogMode::Create | DialogMode::CreateInVault(_) => {
                crate::tr!("add-edit-header-subtitle-create")
            }
            DialogMode::Edit(_) => crate::tr!("add-edit-header-subtitle-edit"),
        };
        let subtitle = gtk4::Label::new(Some(inline_subtitle_text.as_str()));
        subtitle.add_css_class("login-hero-copy");
        subtitle.set_halign(Align::Start);
        subtitle.set_wrap(true);

        header_text.append(&title);
        header_text.append(&subtitle);
        header_box.append(&back_button);
        header_box.append(&header_icon);
        header_box.append(&header_text);
        header_card.set_child(Some(&header_box));

        let form_card = gtk4::Frame::new(None);
        form_card.add_css_class("login-card");

        let form_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();

        let (title_row, title_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-title-label").as_str(),
            crate::tr!("add-edit-field-title-placeholder").as_str(),
            "dialog-title-entry",
        );
        let (category_row, category_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-category-label").as_str(),
            crate::tr!("add-edit-field-category-placeholder").as_str(),
            "dialog-category-entry",
        );
        let (tags_row, tags_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-tags-label").as_str(),
            crate::tr!("add-edit-field-tags-placeholder").as_str(),
            "dialog-tags-entry",
        );

        let type_label = gtk4::Label::new(Some(crate::tr!("add-edit-field-type-label").as_str()));
        type_label.add_css_class("login-field-label");
        type_label.set_halign(Align::Start);

        let type_items =
            gtk4::StringList::new(&["password", "api_token", "ssh_key", "secure_document"]);
        let type_dropdown = gtk4::DropDown::builder().model(&type_items).build();
        type_dropdown.add_css_class("dialog-type-dropdown");
        type_dropdown.set_selected(SecretType::Password.dropdown_index());

        let dynamic_stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::SlideLeftRight)
            .hexpand(true)
            .build();
        dynamic_stack.add_css_class("dialog-dynamic-stack");

        let password_hint = match mode {
            DialogMode::Edit(_) => Some(crate::tr!("add-edit-password-edit-hint")),
            DialogMode::Create | DialogMode::CreateInVault(_) => None,
        };
        let (password_panel, password_entry, password_strength_bar) =
            Self::build_password_panel(password_hint.as_deref());
        let initial_password_snapshot: Rc<std::cell::RefCell<Option<String>>> =
            Rc::new(std::cell::RefCell::new(None));
        dynamic_stack.add_titled(&password_panel, Some("password"), "password");

        let (api_token_panel, api_token_entry, api_provider_entry) = Self::build_api_token_panel();
        dynamic_stack.add_titled(&api_token_panel, Some("api_token"), "api_token");

        let (ssh_key_panel, ssh_private_text, ssh_public_entry, ssh_passphrase_entry) =
            Self::build_ssh_key_panel();
        dynamic_stack.add_titled(&ssh_key_panel, Some("ssh_key"), "ssh_key");

        let (secure_doc_panel, secure_doc_path_entry, secure_doc_mime_entry) =
            Self::build_secure_document_panel();
        dynamic_stack.add_titled(
            &secure_doc_panel,
            Some("secure_document"),
            "secure_document",
        );
        dynamic_stack.set_visible_child_name(Self::stack_name_for_secret_type(SecretType::Password));

        let stack_for_type = dynamic_stack.clone();
        type_dropdown.connect_selected_notify(move |dropdown| {
            match SecretType::from_dropdown_index(dropdown.selected()) {
                Some(secret_type) => {
                    stack_for_type
                        .set_visible_child_name(Self::stack_name_for_secret_type(secret_type));
                }
                None => {
                    warn!(selected_index = dropdown.selected(), "unexpected secret type dropdown index");
                }
            }
        });

        let (username_row, username_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-login-label").as_str(),
            crate::tr!("add-edit-field-login-placeholder").as_str(),
            "dialog-username-entry",
        );
        let (url_row, url_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-url-label").as_str(),
            crate::tr!("add-edit-field-url-placeholder").as_str(),
            "dialog-url-entry",
        );

        let notes_label = gtk4::Label::new(Some(crate::tr!("add-edit-field-notes-label").as_str()));
        notes_label.add_css_class("login-field-label");
        notes_label.set_halign(Align::Start);

        let notes_scrolled = gtk4::ScrolledWindow::builder()
            .min_content_height(120)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        notes_scrolled.add_css_class("dialog-notes-scroll");

        let notes_text = gtk4::TextView::new();
        notes_text.set_wrap_mode(gtk4::WrapMode::WordChar);
        notes_text.set_left_margin(10);
        notes_text.set_right_margin(10);
        notes_text.set_top_margin(10);
        notes_text.set_bottom_margin(10);
        notes_text.add_css_class("dialog-notes-text");
        notes_scrolled.set_child(Some(&notes_text));

        let validity_label =
            gtk4::Label::new(Some(crate::tr!("add-edit-field-validity-label").as_str()));
        validity_label.add_css_class("login-field-label");
        validity_label.set_halign(Align::Start);

        let validity_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();

        let validity_unlimited =
            gtk4::CheckButton::with_label(crate::tr!("add-edit-field-validity-unlimited").as_str());
        validity_unlimited.add_css_class("dialog-validity-check");

        let validity_adjustment = gtk4::Adjustment::new(90.0, 1.0, 3650.0, 1.0, 30.0, 0.0);
        let validity_days = gtk4::SpinButton::builder()
            .adjustment(&validity_adjustment)
            .digits(0)
            .numeric(true)
            .build();
        validity_days.add_css_class("dialog-validity-spin");
        validity_box.append(&validity_unlimited);
        validity_box.append(&validity_days);

        let days_for_toggle = validity_days.clone();
        validity_unlimited.connect_toggled(move |toggle| {
            days_for_toggle.set_sensitive(!toggle.is_active());
        });

        let error_label = gtk4::Label::new(None);
        error_label.add_css_class("login-error");
        error_label.set_halign(Align::Start);
        error_label.set_wrap(true);
        error_label.set_visible(false);

        let button_row = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .halign(Align::End)
            .build();

        let cancel_button = gtk4::Button::with_label(crate::tr!("add-edit-button-cancel").as_str());
        cancel_button.add_css_class("secondary-pill");
        let on_cancel_for_cancel = Rc::clone(&on_cancel);
        cancel_button.connect_clicked(move |_| {
            on_cancel_for_cancel();
        });

        let save_button = gtk4::Button::new();
        save_button.add_css_class("primary-pill");
        let save_button_content = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .halign(Align::Center)
            .build();
        let save_spinner = gtk4::Spinner::new();
        save_spinner.set_visible(false);
        let inline_save_label_text = match mode {
            DialogMode::Create | DialogMode::CreateInVault(_) => {
                crate::tr!("add-edit-button-save-create")
            }
            DialogMode::Edit(_) => crate::tr!("add-edit-button-save-edit"),
        };
        let save_label = gtk4::Label::new(Some(inline_save_label_text.as_str()));
        save_button_content.append(&save_spinner);
        save_button_content.append(&save_label);
        save_button.set_child(Some(&save_button_content));

        {
            let strength = password_strength_bar.clone();
            let dropdown = type_dropdown.clone();
            let btn = save_button.clone();
            let password_for_check = password_entry.clone();
            let initial_password_snapshot_for_check = Rc::clone(&initial_password_snapshot);
            let is_edit_mode = matches!(mode, DialogMode::Edit(_));
            let check: Rc<dyn Fn()> = Rc::new(move || {
                let selected_type = SecretType::from_dropdown_index(dropdown.selected());
                let is_password_type = matches!(selected_type, Some(SecretType::Password));
                let password_text = password_for_check.text();
                let password_raw = password_text.trim().to_string();
                let password_is_filled = !password_raw.is_empty();
                let unchanged_prefilled_password = if is_edit_mode {
                    initial_password_snapshot_for_check
                        .borrow()
                        .as_ref()
                        .map(|value| value == &password_raw)
                        .unwrap_or(false)
                } else {
                    false
                };

                let can_save =
                    if is_password_type && password_is_filled && !unchanged_prefilled_password {
                        // Password field is filled: enforce robuste score
                        strength.last_score() >= 4
                    } else if is_password_type && !password_is_filled && is_edit_mode {
                        // In edit mode with empty password: can update other fields
                        true
                    } else if is_password_type && unchanged_prefilled_password && is_edit_mode {
                        // Existing password only displayed: allow updating other fields
                        true
                    } else if is_password_type && !password_is_filled && !is_edit_mode {
                        // In create mode with empty password: cannot save
                        false
                    } else {
                        // Not password type: always allow
                        true
                    };
                btn.set_sensitive(can_save);
            });
            check();
            let c = Rc::clone(&check);
            password_entry.connect_text_notify(move |_| c());
            let c = Rc::clone(&check);
            type_dropdown.connect_selected_notify(move |_| c());
        }

        let title_for_save = title_entry.clone();
        let category_for_save = category_entry.clone();
        let tags_for_save = tags_entry.clone();
        let type_for_save = type_dropdown.clone();
        let username_for_save = username_entry.clone();
        let url_for_save = url_entry.clone();
        let notes_for_save = notes_text.buffer();
        let validity_unlimited_for_save = validity_unlimited.clone();
        let validity_days_for_save = validity_days.clone();
        let password_for_save = password_entry.clone();
        let api_token_for_save = api_token_entry.clone();
        let api_provider_for_save = api_provider_entry.clone();
        let ssh_private_for_save = ssh_private_text.clone();
        let ssh_public_for_save = ssh_public_entry.clone();
        let ssh_passphrase_for_save = ssh_passphrase_entry.clone();
        let secure_doc_for_save = secure_doc_path_entry.clone();
        let secure_doc_mime_for_save = secure_doc_mime_entry.clone();
        let error_for_save = error_label.clone();
        let spinner_for_save = save_spinner.clone();
        let save_btn_for_save = save_button.clone();
        let secret_for_save = Arc::clone(&secret_service);
        let vault_for_save = Arc::clone(&vault_service);
        let runtime_for_save = runtime_handle.clone();
        let on_saved_for_save = Rc::clone(&on_saved);
        let on_cancel_for_save = Rc::clone(&on_cancel);
        let mode_for_save = mode;
        let admin_master_for_save_seed = admin_master_key.clone();
        let initial_password_snapshot_for_save = Rc::clone(&initial_password_snapshot);
        save_button.connect_clicked(move |_| {
            error_for_save.set_visible(false);
            error_for_save.set_text("");

            let title = title_for_save.text().trim().to_string();
            if title.is_empty() {
                error_for_save.set_text(crate::tr!("add-edit-error-title-required").as_str());
                error_for_save.set_visible(true);
                return;
            }

            let Some(selected_type) = SecretType::from_dropdown_index(type_for_save.selected())
            else {
                warn!(
                    selected_index = type_for_save.selected(),
                    "unexpected secret type dropdown index on save"
                );
                error_for_save.set_text(crate::tr!("add-edit-error-save-failed").as_str());
                error_for_save.set_visible(true);
                return;
            };
            let category = category_for_save.text().trim().to_string();
            let tags_value = tags_for_save.text().trim().to_string();
            let username = username_for_save.text().trim().to_string();
            let url = url_for_save.text().trim().to_string();
            let notes = notes_for_save
                .text(
                    &notes_for_save.start_iter(),
                    &notes_for_save.end_iter(),
                    false,
                )
                .to_string();
            let expires_at = if validity_unlimited_for_save.is_active() {
                None
            } else {
                let days = i64::from(validity_days_for_save.value_as_int());
                if days <= 0 {
                    None
                } else {
                    let ts = OffsetDateTime::now_utc() + Duration::days(days);
                    ts.format(&Rfc3339).ok()
                }
            };
            let metadata_json = {
                let mut metadata = Map::new();
                metadata.insert("category".to_string(), Value::String(category));
                metadata.insert("notes".to_string(), Value::String(notes));
                metadata.insert("login".to_string(), Value::String(username));
                metadata.insert("url".to_string(), Value::String(url));
                metadata.insert(
                    "validity_unlimited".to_string(),
                    Value::Bool(validity_unlimited_for_save.is_active()),
                );
                metadata.insert(
                    "validity_days".to_string(),
                    Value::Number(i64::from(validity_days_for_save.value_as_int()).into()),
                );
                match selected_type {
                    SecretType::ApiToken => {
                        metadata.insert(
                            "provider".to_string(),
                            Value::String(api_provider_for_save.text().trim().to_string()),
                        );
                    }
                    SecretType::SshKey => {
                        metadata.insert(
                            "ssh_public_key".to_string(),
                            Value::String(ssh_public_for_save.text().trim().to_string()),
                        );
                        metadata.insert(
                            "ssh_passphrase".to_string(),
                            Value::String(ssh_passphrase_for_save.text().trim().to_string()),
                        );
                    }
                    SecretType::SecureDocument => {
                        metadata.insert(
                            "document_mime".to_string(),
                            Value::String(secure_doc_mime_for_save.text().trim().to_string()),
                        );
                    }
                    SecretType::Password => {}
                }
                Some(Value::Object(metadata).to_string())
            };

            let mut secret_text = match selected_type {
                SecretType::Password => password_for_save.text().to_string(),
                SecretType::ApiToken => api_token_for_save.text().to_string(),
                SecretType::SshKey => {
                    let buffer = ssh_private_for_save.buffer();
                    buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), false)
                        .to_string()
                }
                SecretType::SecureDocument => secure_doc_for_save.text().to_string(),
            };
            let secret_type = selected_type;

            if matches!(mode_for_save, DialogMode::Edit(_))
                && matches!(selected_type, SecretType::Password)
            {
                if let Some(initial_value) = initial_password_snapshot_for_save.borrow().as_ref() {
                    if secret_text.trim() == initial_value.trim() {
                        secret_text.clear();
                    }
                }
            }

            if matches!(
                mode_for_save,
                DialogMode::Create | DialogMode::CreateInVault(_)
            ) && secret_text.trim().is_empty()
            {
                error_for_save.set_text(crate::tr!("add-edit-error-secret-required").as_str());
                error_for_save.set_visible(true);
                return;
            }

            save_btn_for_save.set_sensitive(false);
            save_spinner.set_visible(true);
            save_spinner.set_spinning(true);

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let secret_service_for_task = Arc::clone(&secret_for_save);
            let vault_service_for_task = Arc::clone(&vault_for_save);
            let runtime_for_task = runtime_for_save.clone();
            let admin_master_for_task = admin_master_for_save_seed.clone();
            let title_for_task = title.clone();
            let title_for_result = title.clone();
            let metadata_for_task = metadata_json.clone();
            let tags_for_task = if tags_value.is_empty() {
                None
            } else {
                Some(tags_value)
            };
            let expires_for_task = expires_at.clone();
            let secret_payload = secret_text.into_bytes();
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    let target_vault_id = match mode_for_save {
                        DialogMode::CreateInVault(vid) => vid,
                        DialogMode::Create => {
                            let vaults = vault_service_for_task
                                .list_user_vaults(admin_user_id)
                                .await?;
                            vaults
                                .into_iter()
                                .next()
                                .ok_or_else(|| {
                                    crate::errors::AppError::NotFound("vault not found".to_string())
                                })?
                                .id
                        }
                        DialogMode::Edit(secret_id) => {
                            let vaults = vault_service_for_task
                                .list_user_vaults(admin_user_id)
                                .await?;
                            let mut found_vault_id: Option<Uuid> = None;
                            for vault in vaults {
                                let items = secret_service_for_task.list_by_vault(vault.id).await?;
                                if items.into_iter().any(|item| item.id == secret_id) {
                                    found_vault_id = Some(vault.id);
                                    break;
                                }
                            }
                            found_vault_id.ok_or_else(|| {
                                crate::errors::AppError::NotFound("secret not found".to_string())
                            })?
                        }
                    };
                    let access = vault_service_for_task
                        .get_vault_access_for_user(admin_user_id, target_vault_id)
                        .await?
                        .ok_or({
                            crate::errors::AppError::Authorization(
                                crate::errors::AccessDeniedReason::VaultAccessDenied,
                            )
                        })?;
                    let is_shared = access.vault.owner_user_id != admin_user_id;
                    if matches!(
                        mode_for_save,
                        DialogMode::Create | DialogMode::CreateInVault(_)
                    ) && is_shared
                        && !access.role.can_admin()
                    {
                        return Err(crate::errors::AppError::Authorization(
                            crate::errors::AccessDeniedReason::VaultSharedCreateDenied,
                        ));
                    }
                    let vault_key = vault_service_for_task
                        .open_vault_for_user(
                            admin_user_id,
                            target_vault_id,
                            SecretBox::new(Box::new(admin_master_for_task.clone())),
                        )
                        .await?;

                    match mode_for_save {
                        DialogMode::Create | DialogMode::CreateInVault(_) => {
                            secret_service_for_task
                                .create_secret(
                                    target_vault_id,
                                    secret_type,
                                    Some(title_for_task),
                                    metadata_for_task,
                                    tags_for_task,
                                    expires_for_task,
                                    SecretBox::new(Box::new(secret_payload)),
                                    vault_key,
                                )
                                .await
                                .map(|_| ())
                        }
                        DialogMode::Edit(secret_id) => {
                            let secret_to_update = if secret_payload.is_empty() {
                                None
                            } else {
                                Some(SecretBox::new(Box::new(secret_payload)))
                            };
                            secret_service_for_task
                                .update_secret(
                                    secret_id,
                                    Some(title_for_task),
                                    metadata_for_task,
                                    tags_for_task,
                                    expires_for_task,
                                    secret_to_update,
                                    vault_key,
                                )
                                .await
                        }
                    }
                });
                let _ = sender.send(result);
            });

            let error_for_result = error_for_save.clone();
            let save_btn_for_result = save_btn_for_save.clone();
            let spinner_for_result = spinner_for_save.clone();
            let on_saved_for_result = Rc::clone(&on_saved_for_save);
            let on_cancel_for_result = Rc::clone(&on_cancel_for_save);
            glib::MainContext::default().spawn_local(async move {
                save_btn_for_result.set_sensitive(true);
                spinner_for_result.set_visible(false);
                spinner_for_result.set_spinning(false);

                match receiver.await {
                    Ok(Ok(_)) => {
                        on_saved_for_result(title_for_result);
                        on_cancel_for_result();
                    }
                    Ok(Err(_)) | Err(_) => {
                        error_for_result
                            .set_text(crate::tr!("add-edit-error-save-failed").as_str());
                        error_for_result.set_visible(true);
                    }
                }
            });
        });

        button_row.append(&cancel_button);
        button_row.append(&save_button);

        form_box.append(&title_row);
        form_box.append(&category_row);
        form_box.append(&tags_row);
        form_box.append(&type_label);
        form_box.append(&type_dropdown);
        form_box.append(&dynamic_stack);
        form_box.append(&username_row);
        form_box.append(&url_row);
        form_box.append(&notes_label);
        form_box.append(&notes_scrolled);
        form_box.append(&validity_label);
        form_box.append(&validity_box);
        form_box.append(&error_label);
        form_box.append(&button_row);

        form_card.set_child(Some(&form_box));
        root.append(&header_card);
        root.append(&form_card);
        container.set_child(Some(&root));

        if let DialogMode::Edit(secret_id) = mode {
            type_dropdown.set_sensitive(false);
            Self::setup_for_edit(
                runtime_handle,
                Arc::clone(&secret_service),
                Arc::clone(&vault_service),
                admin_user_id,
                admin_master_key.clone(),
                show_passwords_in_edit,
                secret_id,
                title_entry.clone(),
                category_entry.clone(),
                tags_entry.clone(),
                type_dropdown.clone(),
                password_entry.clone(),
                Rc::clone(&initial_password_snapshot),
                username_entry.clone(),
                url_entry.clone(),
                notes_text.buffer(),
                validity_unlimited.clone(),
                validity_days.clone(),
                api_provider_entry.clone(),
                ssh_public_entry.clone(),
                ssh_passphrase_entry.clone(),
                secure_doc_mime_entry.clone(),
                error_label.clone(),
            );
        }

        AddEditInlineView { container }
}

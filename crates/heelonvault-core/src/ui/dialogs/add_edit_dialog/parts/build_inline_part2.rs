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

use super::*;
use libadwaita as adw;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_profile_view<
    TUser,
    TTotp,
    TPolicy,
    TBackup,
    TBackupApp,
    TImport,
    TSecret,
    TVault,
>(
    window: adw::ApplicationWindow,
    runtime_handle: Handle,
    user_service: Arc<TUser>,
    user_repo: Arc<heelonvault_core::repositories::user_repository::SqlxUserRepository>,
    crypto_service: Arc<heelonvault_core::services::crypto_service::CryptoServiceImpl>,
    totp_service: Arc<TTotp>,
    auth_policy_service: Arc<TPolicy>,
    backup_service: Arc<TBackup>,
    backup_app_service: Arc<TBackupApp>,
    import_service: Arc<TImport>,
    secret_service: Arc<TSecret>,
    vault_service: Arc<TVault>,
    database_path: PathBuf,
    user_id: Uuid,
    is_admin: bool,
    on_open_users_view: Rc<dyn Fn()>,
    on_open_teams_view: Rc<dyn Fn()>,
    profile_badge: gtk4::MenuButton,
    critical_ops_in_flight: Rc<Cell<u32>>,
    auto_lock_timeout_secs: Rc<Cell<u64>>,
    auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: Rc<Cell<bool>>,
    on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    pin_cache: Rc<RefCell<Option<heelonvault_core::services::pin_cache_service::PinCache>>>,
    show_passwords_in_edit_pref: Rc<Cell<bool>>,
    on_import_completed_refresh: Rc<dyn Fn()>,
    on_language_changed: Rc<dyn Fn()>,
    on_pin_state_changed: Rc<dyn Fn(bool)>,
) -> ProfileViewWidgets
where
    TUser: UserService + Send + Sync + 'static,
    TTotp: TotpService + Send + Sync + 'static,
    TPolicy: AuthPolicyService + Send + Sync + 'static,
    TBackup: BackupService + Send + Sync + 'static,
    TBackupApp: BackupApplicationService + Send + Sync + 'static,
    TImport: ImportService + Send + Sync + 'static,
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let container = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    content.add_css_class("profile-view-content");

    let header = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    header.add_css_class("profile-view-header");
    let back_button = gtk4::Button::with_label(heelonvault_core::tr!("profile-view-back").as_str());
    back_button.add_css_class("flat");
    back_button.set_halign(Align::Start);
    let title = gtk4::Label::new(Some(heelonvault_core::tr!("profile-view-title").as_str()));
    title.add_css_class("title-3");
    title.add_css_class("heading");
    title.set_hexpand(true);
    title.set_halign(Align::Center);
    header.append(&back_button);
    header.append(&title);
    content.append(&header);

    let profile_intro =
        gtk4::Label::new(Some(heelonvault_core::tr!("profile-view-intro").as_str()));
    profile_intro.set_halign(Align::Start);
    profile_intro.set_wrap(true);
    profile_intro.add_css_class("dim-label");
    content.append(&profile_intro);

    let sections_columns = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(18)
        .hexpand(true)
        .homogeneous(true)
        .margin_start(6)
        .margin_end(6)
        .margin_top(6)
        .margin_bottom(0)
        .build();
    sections_columns.add_css_class("profile-sections-columns");

    let sections_left = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .hexpand(true)
        .build();
    sections_left.add_css_class("profile-sections-column");

    let sections_right = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .hexpand(true)
        .build();
    sections_right.add_css_class("profile-sections-column");

    sections_columns.append(&sections_left);
    sections_columns.append(&sections_right);
    content.append(&sections_columns);

    let identity_section = sections::identity::build();
    sections_left.append(&identity_section.frame);

    let password_section = sections::password::build();
    sections_right.append(&password_section.frame);

    let security_section = sections::security_prefs::build(pin_cache.borrow().is_some());
    sections_right.append(&security_section.frame);

    let twofa_section = sections::twofa::build();
    sections_right.append(&twofa_section.group);

    let data_section = sections::data::build();
    sections_left.append(&data_section.frame);

    let admin_section = is_admin.then(|| {
        let section = sections::admin::build(
            Rc::clone(&on_open_users_view),
            Rc::clone(&on_open_teams_view),
        );
        sections_right.append(&section.frame);
        section
    });

    container.set_child(Some(&content));

    let admin_section_for_i18n = admin_section.clone();
    let back_button_for_i18n = back_button.clone();
    let title_for_i18n = title.clone();
    let intro_for_i18n = profile_intro.clone();

    let data_section_for_i18n = data_section.clone();
    let identity_section_for_i18n = identity_section.clone();
    let password_section_for_i18n = password_section.clone();
    let security_section_for_i18n = security_section.clone();
    let twofa_section_for_i18n = twofa_section.clone();

    let on_language_changed_for_i18n = Rc::clone(&on_language_changed);
    let apply_profile_i18n: Rc<dyn Fn()> = Rc::new(move || {
        back_button_for_i18n.set_label(heelonvault_core::tr!("profile-view-back").as_str());
        title_for_i18n.set_text(heelonvault_core::tr!("profile-view-title").as_str());
        intro_for_i18n.set_text(heelonvault_core::tr!("profile-view-intro").as_str());

        identity_section_for_i18n.refresh_i18n();
        password_section_for_i18n.refresh_i18n();
        security_section_for_i18n.refresh_i18n();

        twofa_section_for_i18n.refresh_i18n();

        data_section_for_i18n.refresh_i18n();

        if let Some(admin_section) = admin_section_for_i18n.as_ref() {
            admin_section.refresh_i18n();
        }

        on_language_changed_for_i18n();
    });
    apply_profile_i18n();

    let content_for_compact = content.clone();
    let sections_columns_for_compact = sections_columns.clone();
    let save_row_for_compact = identity_section.save_row.clone();
    let security_actions_for_compact = password_section.actions_row.clone();
    let save_btn_for_compact = identity_section.save_button.clone();
    let change_btn_for_compact = password_section.change_button.clone();
    let rotate_btn_for_compact = password_section.rotate_button.clone();
    let export_btn_for_compact = data_section.export_button.clone();
    let import_btn_for_compact = data_section.import_button.clone();
    container.add_tick_callback(move |widget, _clock| {
        if widget.allocated_width() < 760 {
            content_for_compact.add_css_class("profile-compact");
            sections_columns_for_compact.set_orientation(Orientation::Vertical);
            sections_columns_for_compact.set_homogeneous(false);
            save_row_for_compact.set_orientation(Orientation::Vertical);
            save_row_for_compact.set_halign(Align::Fill);
            security_actions_for_compact.set_orientation(Orientation::Vertical);
            security_actions_for_compact.set_halign(Align::Fill);

            for button in [
                save_btn_for_compact.clone(),
                change_btn_for_compact.clone(),
                rotate_btn_for_compact.clone(),
                export_btn_for_compact.clone(),
                import_btn_for_compact.clone(),
            ] {
                button.set_hexpand(true);
                button.set_halign(Align::Fill);
            }
        } else {
            content_for_compact.remove_css_class("profile-compact");
            sections_columns_for_compact.set_orientation(Orientation::Horizontal);
            sections_columns_for_compact.set_homogeneous(true);
            save_row_for_compact.set_orientation(Orientation::Horizontal);
            save_row_for_compact.set_halign(Align::End);
            security_actions_for_compact.set_orientation(Orientation::Horizontal);
            security_actions_for_compact.set_halign(Align::End);

            for button in [
                save_btn_for_compact.clone(),
                change_btn_for_compact.clone(),
                rotate_btn_for_compact.clone(),
                export_btn_for_compact.clone(),
                import_btn_for_compact.clone(),
            ] {
                button.set_hexpand(false);
                button.set_halign(Align::End);
            }
        }
        glib::ControlFlow::Continue
    });

    let begin_critical_operation: Rc<dyn Fn()> = {
        let counter = Rc::clone(&critical_ops_in_flight);
        let export_btn = data_section.export_button.clone();
        let import_btn = data_section.import_button.clone();
        Rc::new(move || {
            let next = counter.get().saturating_add(1);
            counter.set(next);
            export_btn.set_sensitive(false);
            import_btn.set_sensitive(false);
        })
    };

    let end_critical_operation: Rc<dyn Fn()> = {
        let counter = Rc::clone(&critical_ops_in_flight);
        let export_btn = data_section.export_button.clone();
        let import_btn = data_section.import_button.clone();
        Rc::new(move || {
            let next = counter.get().saturating_sub(1);
            counter.set(next);
            if next == 0 {
                export_btn.set_sensitive(true);
                import_btn.set_sensitive(true);
            }
        })
    };

    let loading_lock = Rc::new(Cell::new(true));
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let service_for_load = Arc::clone(&user_service);
    let totp_for_load = Arc::clone(&totp_service);
    let policy_for_load = Arc::clone(&auth_policy_service);
    let runtime_for_load = runtime_handle.clone();
    std::thread::spawn(move || {
        let result = runtime_for_load.block_on(async move {
            let user = service_for_load.get_user_profile(user_id).await?;
            let delay = policy_for_load
                .get_auto_lock_delay(user.username.as_str())
                .await?;
            let totp_enabled = totp_for_load.is_totp_enabled_for_user_id(user_id).await?;
            Ok::<_, heelonvault_core::errors::AppError>((user, delay, totp_enabled))
        });
        let _ = sender.send(result);
    });

    let username_entry_for_load = identity_section.username_entry.clone();
    let display_entry_for_load = identity_section.display_entry.clone();
    let email_entry_for_load = identity_section.email_entry.clone();
    let auto_lock_for_load = security_section.auto_lock_dropdown.clone();
    let language_dropdown_for_load = identity_section.language_dropdown.clone();
    let show_edit_passwords_for_load = security_section.show_passwords_switch.clone();
    let show_passwords_pref_for_load = Rc::clone(&show_passwords_in_edit_pref);
    let twofa_stack_for_load = twofa_section.stack.clone();
    let twofa_disable_confirm_row_for_load = twofa_section.disable_confirm_row.clone();
    let twofa_badge_for_load = twofa_section.state_badge.clone();
    let loading_lock_for_load = Rc::clone(&loading_lock);
    let profile_status_for_load = identity_section.status_label.clone();
    let apply_profile_i18n_for_load = Rc::clone(&apply_profile_i18n);
    glib::MainContext::default().spawn_local(async move {
        match receiver.await {
            Ok(Ok((user, delay, totp_enabled))) => {
                let language = user.preferred_language.trim().to_ascii_lowercase();
                let _ = heelonvault_core::i18n::set_language(language.as_str());
                apply_profile_i18n_for_load();
                username_entry_for_load.set_text(user.username.as_str());
                display_entry_for_load.set_text(user.display_name.as_deref().unwrap_or_default());
                email_entry_for_load.set_text(user.email.as_deref().unwrap_or_default());
                language_dropdown_for_load.set_selected(if language.starts_with("en") {
                    1
                } else {
                    0
                });
                let selected = match delay {
                    1 => 0,
                    5 => 1,
                    30 => 2,
                    0 => 3,
                    _ => 1,
                };
                auto_lock_for_load.set_selected(selected);
                show_edit_passwords_for_load.set_active(user.show_passwords_in_edit);
                show_passwords_pref_for_load.set(user.show_passwords_in_edit);
                twofa_stack_for_load.set_visible_child_name(if totp_enabled {
                    "enabled"
                } else {
                    "disabled"
                });
                MainWindow::set_twofa_badge_state(&twofa_badge_for_load, totp_enabled);
                twofa_disable_confirm_row_for_load.set_visible(false);
                loading_lock_for_load.set(false);
            }
            _ => {
                loading_lock_for_load.set(false);
                MainWindow::set_inline_status(
                    &profile_status_for_load,
                    heelonvault_core::tr!("profile-status-load-failed").as_str(),
                    "error",
                );
            }
        }
    });

    let policy_for_delay = Arc::clone(&auth_policy_service);
    let runtime_for_delay = runtime_handle.clone();
    let username_for_delay = identity_section.username_entry.clone();
    let loading_lock_for_delay = Rc::clone(&loading_lock);
    let window_for_delay = window.clone();
    let security_status_for_delay = security_section.status_label.clone();
    let session_for_delay = Rc::clone(&session_master_key);
    security_section
        .auto_lock_dropdown
        .connect_selected_notify(move |dropdown| {
            if loading_lock_for_delay.get() {
                return;
            }

            let username = username_for_delay.text().trim().to_string();
            if username.is_empty() {
                return;
            }

            let mins = match dropdown.selected() {
                0 => 1,
                1 => 5,
                2 => 30,
                3 => 0,
                _ => 5,
            };
            if mins == 0 {
                MainWindow::show_feedback_dialog(
                    &window_for_delay,
                    heelonvault_core::tr!("profile-auto-lock-never-warning-title").as_str(),
                    heelonvault_core::tr!("profile-auto-lock-never-warning-body").as_str(),
                );
            }
            let status_message = if mins == 0 {
                heelonvault_core::tr!("profile-status-lock-delay-warning")
            } else {
                heelonvault_core::tr!("profile-status-lock-delay-updating")
            };
            MainWindow::set_inline_status(
                &security_status_for_delay,
                status_message.as_str(),
                if mins == 0 { "error" } else { "loading" },
            );

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let runtime_for_task = runtime_for_delay.clone();
            let policy_for_task = Arc::clone(&policy_for_delay);
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    policy_for_task
                        .update_auto_lock_delay(username.as_str(), mins)
                        .await
                });
                let _ = sender.send((mins, result));
            });

            let security_status_for_result = security_status_for_delay.clone();
            let auto_lock_timeout_for_result = Rc::clone(&auto_lock_timeout_secs);
            let auto_lock_source_for_result = Rc::clone(&auto_lock_source);
            let auto_lock_armed_for_result = Rc::clone(&auto_lock_armed);
            let on_auto_lock_for_result = Rc::clone(&on_auto_lock);
            let session_for_result = Rc::clone(&session_for_delay);
            let window_for_result = window_for_delay.clone();
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok((updated_mins, Ok(()))) => {
                        auto_lock_timeout_for_result.set((updated_mins as u64).saturating_mul(60));
                        if updated_mins == 0 {
                            auto_lock_armed_for_result.set(false);
                            if let Some(source_id) = auto_lock_source_for_result.borrow_mut().take()
                            {
                                source_id.remove();
                            }
                        } else if auto_lock_armed_for_result.get() {
                            auto_lock::reset_auto_lock_timer(
                                &window_for_result,
                                &auto_lock_source_for_result,
                                &auto_lock_armed_for_result,
                                auto_lock_timeout_for_result.get(),
                                &on_auto_lock_for_result,
                                &session_for_result,
                            );
                        }
                        MainWindow::set_inline_status(
                            &security_status_for_result,
                            heelonvault_core::tr!("profile-status-lock-delay-updated").as_str(),
                            "success",
                        );
                    }
                    _ => {
                        MainWindow::set_inline_status(
                            &security_status_for_result,
                            heelonvault_core::tr!("profile-status-lock-delay-failed").as_str(),
                            "error",
                        );
                    }
                }
            });
        });

    let service_for_profile_save = Arc::clone(&user_service);
    let runtime_for_profile_save = runtime_handle.clone();
    let display_for_save = identity_section.display_entry.clone();
    let email_for_save = identity_section.email_entry.clone();
    let language_dropdown_for_save = identity_section.language_dropdown.clone();
    let current_email_pw_for_save = identity_section.current_email_pw_entry.clone();
    let show_edit_passwords_for_save = security_section.show_passwords_switch.clone();
    let show_passwords_pref_for_save = Rc::clone(&show_passwords_in_edit_pref);
    let profile_badge_for_save = profile_badge.clone();
    let profile_status_for_save = identity_section.status_label.clone();
    let apply_profile_i18n_for_save = Rc::clone(&apply_profile_i18n);
    let service_for_toggle = Arc::clone(&user_service);
    let runtime_for_toggle = runtime_handle.clone();
    let profile_status_for_toggle = security_section.status_label.clone();
    let show_passwords_pref_for_toggle = Rc::clone(&show_passwords_in_edit_pref);
    security_section
        .show_passwords_switch
        .connect_active_notify(move |switch_widget| {
            let enabled = switch_widget.is_active();
            MainWindow::set_inline_status(
                &profile_status_for_toggle,
                heelonvault_core::tr!("profile-status-show-passwords-updating").as_str(),
                "loading",
            );

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let runtime_for_task = runtime_for_toggle.clone();
            let service_for_task = Arc::clone(&service_for_toggle);
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    service_for_task
                        .update_show_passwords_in_edit(user_id, enabled)
                        .await
                });
                let _ = sender.send((enabled, result));
            });

            let profile_status_for_result = profile_status_for_toggle.clone();
            let show_passwords_pref_for_result = Rc::clone(&show_passwords_pref_for_toggle);
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok((_, Ok(user))) => {
                        show_passwords_pref_for_result.set(user.show_passwords_in_edit);
                        MainWindow::set_inline_status(
                            &profile_status_for_result,
                            heelonvault_core::tr!("profile-status-show-passwords-updated").as_str(),
                            "success",
                        );
                    }
                    _ => {
                        MainWindow::set_inline_status(
                            &profile_status_for_result,
                            heelonvault_core::tr!("profile-status-show-passwords-failed").as_str(),
                            "error",
                        );
                    }
                }
            });
        });
    // ── PIN setup button handler ─────────────────────────────────────────────
    let win_for_pin = window.clone();
    let session_for_pin = Rc::clone(&session_master_key);
    let pin_cache_for_setup = Rc::clone(&pin_cache);
    let pin_status_badge_for_setup = security_section.pin_status_badge.clone();
    let on_pin_state_for_setup = Rc::clone(&on_pin_state_changed);
    security_section.pin_setup_button.connect_clicked(move |_| {
        let key_snapshot = {
            let k = session_for_pin.borrow();
            if k.is_empty() {
                return;
            }
            secrecy::SecretBox::new(Box::new(k.clone()))
        };
        let currently_active = pin_cache_for_setup.borrow().is_some();
        let pin_cache_for_created = Rc::clone(&pin_cache_for_setup);
        let badge_for_created = pin_status_badge_for_setup.clone();
        let on_pin_state_for_created = Rc::clone(&on_pin_state_for_setup);
        let pin_cache_for_disabled = Rc::clone(&pin_cache_for_setup);
        let badge_for_disabled = pin_status_badge_for_setup.clone();
        let on_pin_state_for_disabled = Rc::clone(&on_pin_state_for_setup);
        let dialog = crate::ui::dialogs::pin_setup_dialog::PinSetupDialog::new(
            &win_for_pin,
            key_snapshot,
            user_id,
            move |new_cache| {
                *pin_cache_for_created.borrow_mut() = Some(new_cache);
                badge_for_created.set_text(heelonvault_core::tr!("pin-status-active").as_str());
                badge_for_created.remove_css_class("status-role-disabled");
                badge_for_created.add_css_class("status-role-user");
                on_pin_state_for_created(true);
            },
            move || {
                let _ = pin_cache_for_disabled.borrow_mut().take();
                badge_for_disabled.set_text(heelonvault_core::tr!("pin-status-inactive").as_str());
                badge_for_disabled.remove_css_class("status-role-user");
                badge_for_disabled.add_css_class("status-role-disabled");
                on_pin_state_for_disabled(false);
            },
            currently_active,
        );
        dialog.present();
    });
    identity_section.save_button.connect_clicked(move |_| {
        MainWindow::set_inline_status(
            &profile_status_for_save,
            heelonvault_core::tr!("profile-status-saving").as_str(),
            "loading",
        );
        let payload = heelonvault_core::services::user_service::UserProfileUpdate {
            email: {
                let value = email_for_save.text().trim().to_string();
                if value.is_empty() { None } else { Some(value) }
            },
            display_name: {
                let value = display_for_save.text().trim().to_string();
                if value.is_empty() { None } else { Some(value) }
            },
            preferred_language: Some(if language_dropdown_for_save.selected() == 1 {
                "en".to_string()
            } else {
                "fr".to_string()
            }),
            show_passwords_in_edit: Some(show_edit_passwords_for_save.is_active()),
            current_password: {
                let value = current_email_pw_for_save.text().trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(SecretBox::new(Box::new(value.into_bytes())))
                }
            },
        };

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_profile_save.clone();
        let service_for_task = Arc::clone(&service_for_profile_save);
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                service_for_task.update_user_profile(user_id, payload).await
            });
            let _ = sender.send(result);
        });

        let badge_for_result = profile_badge_for_save.clone();
        let profile_status_for_result = profile_status_for_save.clone();
        let show_passwords_pref_for_result = Rc::clone(&show_passwords_pref_for_save);
        let apply_profile_i18n_for_result = Rc::clone(&apply_profile_i18n_for_save);
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(user)) => {
                    let _ = heelonvault_core::i18n::set_language(user.preferred_language.as_str());
                    apply_profile_i18n_for_result();
                    let display = user
                        .display_name
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or(user.username.clone());
                    show_passwords_pref_for_result.set(user.show_passwords_in_edit);
                    badge_for_result.set_label(
                        heelonvault_core::i18n::tr_args(
                            "main-connected-label",
                            &[(
                                "name",
                                heelonvault_core::i18n::I18nArg::Str(display.as_str()),
                            )],
                        )
                        .as_str(),
                    );
                    MainWindow::set_inline_status(
                        &profile_status_for_result,
                        heelonvault_core::tr!("profile-status-saved").as_str(),
                        "success",
                    );
                }
                _ => {
                    MainWindow::set_inline_status(
                        &profile_status_for_result,
                        heelonvault_core::tr!("profile-status-save-failed").as_str(),
                        "error",
                    );
                }
            }
        });
    });

    let service_for_pw_change = Arc::clone(&user_service);
    let runtime_for_pw_change = runtime_handle.clone();
    let current_pw_for_change = password_section.current_entry.clone();
    let security_status_for_pw_change = password_section.status_label.clone();
    password_section.change_button.connect_clicked(move |_| {
        MainWindow::set_inline_status(
            &security_status_for_pw_change,
            heelonvault_core::tr!("profile-status-password-change-ready").as_str(),
            "success",
        );
        current_pw_for_change.grab_focus();
    });

    let service_for_rotate = Arc::clone(&service_for_pw_change);
    let runtime_for_rotate = runtime_for_pw_change.clone();
    let current_pw_for_rotate = password_section.current_entry.clone();
    let new_pw_for_rotate = password_section.new_entry.clone();
    let confirm_pw_for_rotate = password_section.confirm_entry.clone();
    let security_status_for_rotate = password_section.status_label.clone();
    password_section.rotate_button.connect_clicked(move |_| {
        let current_raw = current_pw_for_rotate.text().trim().to_string();
        let new_raw = new_pw_for_rotate.text().trim().to_string();
        let confirm_raw = confirm_pw_for_rotate.text().trim().to_string();
        if current_raw.is_empty() || new_raw.is_empty() || confirm_raw.is_empty() {
            MainWindow::set_inline_status(
                &security_status_for_rotate,
                heelonvault_core::tr!("profile-status-password-fields-required").as_str(),
                "error",
            );
            return;
        }
        if new_raw != confirm_raw {
            MainWindow::set_inline_status(
                &security_status_for_rotate,
                heelonvault_core::tr!("profile-status-password-confirm-mismatch").as_str(),
                "error",
            );
            return;
        }

        MainWindow::set_inline_status(
            &security_status_for_rotate,
            heelonvault_core::tr!("profile-status-password-updating").as_str(),
            "loading",
        );

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_rotate.clone();
        let service_for_task = Arc::clone(&service_for_rotate);
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                service_for_task
                    .change_master_password(
                        user_id,
                        SecretBox::new(Box::new(current_raw.into_bytes())),
                        SecretBox::new(Box::new(new_raw.into_bytes())),
                    )
                    .await
            });
            let _ = sender.send(result);
        });

        let current_for_result = current_pw_for_rotate.clone();
        let new_for_result = new_pw_for_rotate.clone();
        let confirm_for_result = confirm_pw_for_rotate.clone();
        let security_status_for_result = security_status_for_rotate.clone();
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(())) => {
                    current_for_result.set_text("");
                    new_for_result.set_text("");
                    confirm_for_result.set_text("");
                    MainWindow::set_inline_status(
                        &security_status_for_result,
                        heelonvault_core::tr!("profile-status-password-updated").as_str(),
                        "success",
                    );
                }
                _ => {
                    MainWindow::set_inline_status(
                        &security_status_for_result,
                        heelonvault_core::tr!("profile-status-password-failed").as_str(),
                        "error",
                    );
                }
            }
        });
    });

    handlers::twofa::setup(
        &twofa_section,
        &identity_section,
        handlers::twofa::TwoFaHandlerDeps {
            runtime_handle: runtime_handle.clone(),
            totp_service: Arc::clone(&totp_service),
            user_id,
        },
    );

    handlers::data::setup(
        &data_section,
        handlers::data::DataHandlerDeps {
            window: window.clone(),
            runtime_handle: runtime_handle.clone(),
            backup_service: Arc::clone(&backup_service),
            backup_app_service: Arc::clone(&backup_app_service),
            import_service: Arc::clone(&import_service),
            secret_service: Arc::clone(&secret_service),
            vault_service: Arc::clone(&vault_service),
            user_repo: Arc::clone(&user_repo),
            crypto_service: Arc::clone(&crypto_service),
            database_path: database_path.clone(),
            user_id,
            is_admin,
            session_master_key: Rc::clone(&session_master_key),
            on_import_completed_refresh: Rc::clone(&on_import_completed_refresh),
            begin_critical_operation: Rc::clone(&begin_critical_operation),
            end_critical_operation: Rc::clone(&end_critical_operation),
        },
    );

    ProfileViewWidgets {
        container,
        back_button,
    }
}

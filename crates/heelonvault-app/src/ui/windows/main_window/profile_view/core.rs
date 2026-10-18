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
    entitlements: crate::ui::entitlements::Entitlements,
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

    // One centred column read top to bottom: identity, protection, session, preferences,
    // data, administration. Each setting is one row; complex actions expand in place.
    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(24)
        .margin_top(20)
        .margin_bottom(32)
        .margin_start(20)
        .margin_end(20)
        .build();
    content.add_css_class("profile-view-content");
    let clamp = adw::Clamp::builder()
        .maximum_size(760)
        .tightening_threshold(600)
        .child(&content)
        .build();
    container.set_child(Some(&clamp));

    let header = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    header.add_css_class("profile-view-header");
    let back_button = gtk4::Button::with_label(heelonvault_core::tr!("profile-view-back").as_str());
    back_button.add_css_class("flat");
    back_button.set_halign(Align::Start);
    let title = gtk4::Label::new(Some(heelonvault_core::tr!("profile-view-title").as_str()));
    title.add_css_class("title-2");
    title.set_hexpand(true);
    title.set_xalign(0.0);
    header.append(&back_button);
    header.append(&title);
    content.append(&header);

    let summary_section = sections::summary::build(entitlements);
    content.append(&summary_section.frame);

    let identity_section = sections::identity::build();
    content.append(&identity_section.frame);

    let password_section = sections::password::build();
    let security_section = sections::security_prefs::build(pin_cache.borrow().is_some());
    let twofa_section = sections::twofa::build();
    let protection_group = adw::PreferencesGroup::new();
    protection_group.add_css_class("profile-group");
    protection_group.add(&password_section.expander);
    protection_group.add(&twofa_section.expander);
    protection_group.add(&security_section.pin_row);
    content.append(&protection_group);

    content.append(&security_section.frame);

    let preferences_section = sections::preferences::build();
    content.append(&preferences_section.frame);

    let data_section = sections::data::build(entitlements.can_export_backup());
    content.append(&data_section.frame);

    let admin_section = sections::admin::build(
        entitlements,
        Rc::clone(&on_open_users_view),
        Rc::clone(&on_open_teams_view),
    );
    if let Some(section) = admin_section.as_ref() {
        content.append(&section.frame);
    }

    let admin_section_for_i18n = admin_section.clone();
    let back_button_for_i18n = back_button.clone();
    let title_for_i18n = title.clone();
    let protection_group_for_i18n = protection_group.clone();
    let summary_section_for_i18n = summary_section.clone();
    let data_section_for_i18n = data_section.clone();
    let identity_section_for_i18n = identity_section.clone();
    let password_section_for_i18n = password_section.clone();
    let security_section_for_i18n = security_section.clone();
    let twofa_section_for_i18n = twofa_section.clone();
    let preferences_section_for_i18n = preferences_section.clone();

    let on_language_changed_for_i18n = Rc::clone(&on_language_changed);
    let apply_profile_i18n: Rc<dyn Fn()> = Rc::new(move || {
        back_button_for_i18n.set_label(heelonvault_core::tr!("profile-view-back").as_str());
        title_for_i18n.set_text(heelonvault_core::tr!("profile-view-title").as_str());
        protection_group_for_i18n
            .set_title(heelonvault_core::tr!("profile-section-protection").as_str());

        summary_section_for_i18n.refresh_i18n();
        identity_section_for_i18n.refresh_i18n();
        password_section_for_i18n.refresh_i18n();
        security_section_for_i18n.refresh_i18n();
        twofa_section_for_i18n.refresh_i18n();
        preferences_section_for_i18n.refresh_i18n();
        data_section_for_i18n.refresh_i18n();
        if let Some(admin_section) = admin_section_for_i18n.as_ref() {
            admin_section.refresh_i18n();
        }

        on_language_changed_for_i18n();
    });
    apply_profile_i18n();

    // Protection checklist of the header, recomputed from the widgets that own each state.
    let refresh_protection: Rc<dyn Fn()> = {
        let summary = summary_section.clone();
        let twofa_stack = twofa_section.stack.clone();
        let pin_cache = Rc::clone(&pin_cache);
        let security = security_section.clone();
        Rc::new(move || {
            summary.set_protection([
                twofa_stack.visible_child_name().as_deref() == Some("enabled"),
                pin_cache.borrow().is_some(),
                security.auto_lock_enabled(),
            ]);
        })
    };
    {
        let refresh = Rc::clone(&refresh_protection);
        twofa_section
            .stack
            .connect_visible_child_name_notify(move |_| refresh());
        let refresh = Rc::clone(&refresh_protection);
        security_section
            .pin_status_badge
            .connect_label_notify(move |_| refresh());
    }
    refresh_protection();

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

    // Last persisted values: setting a widget programmatically must not persist it again.
    let applied_auto_lock: Rc<Cell<Option<i64>>> = Rc::new(Cell::new(None));
    let applied_language: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let identity_for_load = identity_section.clone();
    let summary_for_load = summary_section.clone();
    let preferences_for_load = preferences_section.clone();
    let applied_auto_lock_for_load = Rc::clone(&applied_auto_lock);
    let applied_language_for_load = Rc::clone(&applied_language);
    let refresh_protection_for_load = Rc::clone(&refresh_protection);
    let auto_lock_for_load = security_section.auto_lock_dropdown.clone();
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
                let display_name = user.display_name.clone().unwrap_or_default();
                let email = user.email.clone().unwrap_or_default();
                identity_for_load
                    .username_entry
                    .set_text(user.username.as_str());
                identity_for_load
                    .display_entry
                    .set_text(display_name.as_str());
                identity_for_load.email_entry.set_text(email.as_str());
                identity_for_load.mark_saved(display_name.as_str(), email.as_str());
                summary_for_load.set_identity(
                    user.username.as_str(),
                    user.display_name.as_deref(),
                    user.email.as_deref(),
                );
                *applied_language_for_load.borrow_mut() = language.clone();
                preferences_for_load.select_language(language.as_str());
                // Unknown stored delays fall back to the 5-minute default.
                let selected = sections::security_prefs::AUTO_LOCK_MINUTES
                    .iter()
                    .position(|minutes| *minutes == delay)
                    .unwrap_or(1);
                applied_auto_lock_for_load.set(
                    sections::security_prefs::AUTO_LOCK_MINUTES
                        .get(selected)
                        .copied(),
                );
                auto_lock_for_load.set_selected(selected as u32);
                refresh_protection_for_load();
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
    let applied_auto_lock_for_delay = Rc::clone(&applied_auto_lock);
    let refresh_protection_for_delay = Rc::clone(&refresh_protection);
    // Set once the user explicitly accepted "never"; consumed by the next selection.
    let never_confirmed = Rc::new(Cell::new(false));
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

            let Some(mins) = sections::security_prefs::AUTO_LOCK_MINUTES
                .get(dropdown.selected() as usize)
                .copied()
            else {
                return;
            };
            // Model rebuilds (language change) re-select the same value: nothing to persist.
            if applied_auto_lock_for_delay.get() == Some(mins) {
                return;
            }
            // Disabling auto-lock is confirmed before it is applied, not reported afterwards.
            if mins == 0 && !never_confirmed.replace(false) {
                let previous = applied_auto_lock_for_delay
                    .get()
                    .and_then(|applied| {
                        sections::security_prefs::AUTO_LOCK_MINUTES
                            .iter()
                            .position(|minutes| *minutes == applied)
                    })
                    .unwrap_or(1);
                let dialog = adw::MessageDialog::new(
                    Some(&window_for_delay),
                    Some(heelonvault_core::tr!("profile-auto-lock-never-warning-title").as_str()),
                    Some(heelonvault_core::tr!("profile-auto-lock-never-warning-body").as_str()),
                );
                dialog.add_response("cancel", heelonvault_core::tr!("common-cancel").as_str());
                dialog.add_response(
                    "disable",
                    heelonvault_core::tr!("profile-auto-lock-never-confirm").as_str(),
                );
                dialog.set_response_appearance("disable", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                let dropdown = dropdown.clone();
                let never_confirmed = Rc::clone(&never_confirmed);
                // Show the previous value while the user decides.
                dropdown.set_selected(previous as u32);
                dialog.connect_response(None, move |_, response| {
                    if response == "disable" {
                        never_confirmed.set(true);
                        dropdown.set_selected(
                            (sections::security_prefs::AUTO_LOCK_MINUTES.len() - 1) as u32,
                        );
                    }
                });
                dialog.present();
                return;
            }
            applied_auto_lock_for_delay.set(Some(mins));
            refresh_protection_for_delay();
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
            let applied_auto_lock_for_result = Rc::clone(&applied_auto_lock_for_delay);
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
                        applied_auto_lock_for_result.set(None);
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
    let preferences_for_save = preferences_section.clone();
    let identity_for_save = identity_section.clone();
    let summary_for_save = summary_section.clone();
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
    // ── Language: applied as soon as it is chosen ────────────────────────────
    // Only the language changes: the payload carries the *saved* identity, not unsaved edits
    // of the account form (an empty field would otherwise clear the stored value).
    let service_for_language = Arc::clone(&user_service);
    let runtime_for_language = runtime_handle.clone();
    let loading_lock_for_language = Rc::clone(&loading_lock);
    let applied_language_for_change = Rc::clone(&applied_language);
    let identity_for_language = identity_section.clone();
    let show_passwords_for_language = security_section.show_passwords_switch.clone();
    let apply_i18n_for_language = Rc::clone(&apply_profile_i18n);
    let preferences_for_language = preferences_section.clone();
    let window_for_language = window.clone();
    preferences_section
        .language_row
        .connect_selected_notify(move |_| {
            if loading_lock_for_language.get() {
                return;
            }
            let language = preferences_for_language.selected_language();
            if applied_language_for_change.borrow().starts_with(language) {
                return;
            }
            *applied_language_for_change.borrow_mut() = language.to_string();
            let saved = identity_for_language.saved.borrow().clone();
            let payload = heelonvault_core::services::user_service::UserProfileUpdate {
                email: Some(saved.email),
                display_name: Some(saved.display_name),
                preferred_language: Some(language.to_string()),
                show_passwords_in_edit: Some(show_passwords_for_language.is_active()),
                current_password: None,
            };
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let runtime_for_task = runtime_for_language.clone();
            let service_for_task = Arc::clone(&service_for_language);
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    service_for_task.update_user_profile(user_id, payload).await
                });
                let _ = sender.send(result);
            });
            let apply_i18n = Rc::clone(&apply_i18n_for_language);
            let applied_language = Rc::clone(&applied_language_for_change);
            let window = window_for_language.clone();
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(user)) => {
                        let _ =
                            heelonvault_core::i18n::set_language(user.preferred_language.as_str());
                        apply_i18n();
                    }
                    _ => {
                        applied_language.borrow_mut().clear();
                        MainWindow::show_feedback_dialog(
                            &window,
                            heelonvault_core::tr!("profile-status-save-failed").as_str(),
                            heelonvault_core::tr!("profile-language-save-failed").as_str(),
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
            preferred_language: Some(preferences_for_save.selected_language().to_string()),
            show_passwords_in_edit: Some(show_edit_passwords_for_save.is_active()),
            // Passwords are used exactly as typed, like on the login screen.
            current_password: {
                let value = current_email_pw_for_save.text().to_string();
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
        let identity_for_result = identity_for_save.clone();
        let summary_for_result = summary_for_save.clone();
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(user)) => {
                    let _ = heelonvault_core::i18n::set_language(user.preferred_language.as_str());
                    apply_profile_i18n_for_result();
                    identity_for_result.mark_saved(
                        user.display_name.as_deref().unwrap_or_default(),
                        user.email.as_deref().unwrap_or_default(),
                    );
                    summary_for_result.set_identity(
                        user.username.as_str(),
                        user.display_name.as_deref(),
                        user.email.as_deref(),
                    );
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
    let service_for_rotate = Arc::clone(&service_for_pw_change);
    let runtime_for_rotate = runtime_for_pw_change.clone();
    let current_pw_for_rotate = password_section.current_entry.clone();
    let new_pw_for_rotate = password_section.new_entry.clone();
    let confirm_pw_for_rotate = password_section.confirm_entry.clone();
    let security_status_for_rotate = password_section.status_label.clone();
    let session_for_rotate = Rc::clone(&session_master_key);
    let pin_cache_for_rotate = Rc::clone(&pin_cache);
    let pin_badge_for_rotate = security_section.pin_status_badge.clone();
    let on_pin_state_for_rotate = Rc::clone(&on_pin_state_changed);
    password_section.rotate_button.connect_clicked(move |_| {
        // Exactly as typed: the login screen does not trim, so trimming here could store a
        // password the user can no longer type.
        let current_raw = current_pw_for_rotate.text().to_string();
        let new_raw = new_pw_for_rotate.text().to_string();
        let confirm_raw = confirm_pw_for_rotate.text().to_string();
        if current_raw.is_empty() || new_raw.is_empty() || confirm_raw.is_empty() {
            MainWindow::set_inline_status(
                &security_status_for_rotate,
                heelonvault_core::tr!("profile-status-password-fields-required").as_str(),
                "error",
            );
            return;
        }
        if new_raw.chars().count() < sections::password::MIN_MASTER_PASSWORD_CHARS {
            MainWindow::set_inline_status(
                &security_status_for_rotate,
                heelonvault_core::i18n::tr_args(
                    "profile-status-password-too-short",
                    &[(
                        "min",
                        heelonvault_core::i18n::I18nArg::Num(
                            sections::password::MIN_MASTER_PASSWORD_CHARS as i64,
                        ),
                    )],
                )
                .as_str(),
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
        let session_for_result = Rc::clone(&session_for_rotate);
        let pin_cache_for_result = Rc::clone(&pin_cache_for_rotate);
        let pin_badge_for_result = pin_badge_for_rotate.clone();
        let on_pin_state_for_result = Rc::clone(&on_pin_state_for_rotate);
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(new_master_key)) => {
                    let key_changed = {
                        let mut session_key = session_for_result.borrow_mut();
                        let new_key = secrecy::ExposeSecret::expose_secret(&new_master_key);
                        let changed = session_key.as_slice() != new_key.as_slice();
                        if changed {
                            zeroize::Zeroize::zeroize(&mut *session_key);
                            *session_key = new_key.clone();
                        }
                        changed
                    };
                    // Only a changed key invalidates the PIN cache, which wraps the session key.
                    let pin_was_active =
                        key_changed && pin_cache_for_result.borrow_mut().take().is_some();
                    if pin_was_active {
                        pin_badge_for_result
                            .set_text(heelonvault_core::tr!("pin-status-inactive").as_str());
                        pin_badge_for_result.remove_css_class("status-role-user");
                        pin_badge_for_result.add_css_class("status-role-disabled");
                        on_pin_state_for_result(false);
                    }
                    current_for_result.set_text("");
                    new_for_result.set_text("");
                    confirm_for_result.set_text("");
                    let status_key = if pin_was_active {
                        "profile-status-password-updated-pin-reset"
                    } else {
                        "profile-status-password-updated"
                    };
                    MainWindow::set_inline_status(
                        &security_status_for_result,
                        heelonvault_core::tr!(status_key).as_str(),
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
            session_master_key: Rc::clone(&session_master_key),
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
            can_export_backup: entitlements.can_export_backup(),
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

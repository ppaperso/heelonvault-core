{
        let initial_launch = Self::initial_window_launch();
        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title("HeelonVault")
            .default_width(initial_launch.width)
            .default_height(initial_launch.height)
            .build();
        window.add_css_class("app-window");
        window.add_css_class("main-window");
        window.set_icon_name(Some("heelonvault"));
        if initial_launch.fullscreen {
            window.fullscreen();
        }

        let auto_lock_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let auto_lock_armed = Rc::new(Cell::new(false));
        let auto_lock_timeout_secs = Rc::new(Cell::new(Self::DEFAULT_AUTO_LOCK_TIMEOUT_SECS));
        let session_master_key = Rc::new(RefCell::new(admin_master_key));
        let active_vault_id: Rc<RefCell<Option<Uuid>>> = Rc::new(RefCell::new(None));
        let on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_logout: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let critical_ops_in_flight = Rc::new(Cell::new(0_u32));
        let audit_report_service = Arc::new(AuditReportService::new(
            Arc::clone(&license_service),
            runtime_handle.clone(),
            database_pool.clone(),
        ));
        let report_customer_name = Self::professional_customer_name(license_badge_text.as_str())
            .unwrap_or_else(|| "CLIENT".to_string());

        let header_bar = adw::HeaderBar::new();
        header_bar.add_css_class("main-headerbar");
        header_bar.set_show_start_title_buttons(false);
        header_bar.set_show_end_title_buttons(true);
        header_bar.set_decoration_layout(Some(":close"));

        let on_logout_for_close = Rc::clone(&on_logout);
        let critical_ops_for_close = Rc::clone(&critical_ops_in_flight);
        window.connect_close_request(move |win| {
            if critical_ops_for_close.get() > 0 {
                Self::show_feedback_dialog(
					win,
					"Opération en cours",
					"Une opération d'import/export est en cours. Attendez la fin avant de fermer la fenêtre.",
				);
                return glib::Propagation::Stop;
            }

            window_sizing::persist_main_window_state(
                win.width(),
                win.height(),
                win.is_fullscreen(),
            );

            if let Some(callback) = on_logout_for_close.borrow().as_ref() {
                callback();
            }
            glib::Propagation::Stop
        });

        let root = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(0)
            .build();
        let toast_overlay = adw::ToastOverlay::new();

        let title_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();
        let logo =
            gtk4::Image::from_resource("/com/heelonvault/rust/icons/Logo_Heelonys_transparent.png");
        logo.set_pixel_size(22);
        logo.add_css_class("main-title-logo");
        let title_label = gtk4::Label::new(Some("HeelonVault"));
        title_label.add_css_class("title-3");
        title_label.add_css_class("main-title");
        let header_plan_badge = gtk4::Label::new(Some(&format!("v{}", env!("CARGO_PKG_VERSION"))));
        header_plan_badge.add_css_class("beta-badge");
        header_plan_badge.add_css_class("header-beta-badge");
        let header_license_badge = Self::build_header_license_badge(license_badge_text.as_str());
        title_box.append(&logo);
        title_box.append(&title_label);
        title_box.append(&header_plan_badge);
        title_box.append(&header_license_badge);
        header_bar.set_title_widget(Some(&title_box));

        let profile_button = gtk4::MenuButton::new();
        profile_button.add_css_class("header-badge");
        profile_button.set_label(
            crate::i18n::tr_args(
                "main-connected-label",
                &[(
                    "name",
                    crate::i18n::I18nArg::Str(connected_identity_label.as_str()),
                )],
            )
            .as_str(),
        );
        profile_button.set_tooltip_text(Some(crate::tr!("main-last-logins-tooltip").as_str()));

        let profile_popover = gtk4::Popover::new();
        profile_popover.set_has_arrow(true);
        profile_popover.set_autohide(true);
        let profile_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();
        profile_box.add_css_class("profile-login-history-popover");

        let profile_title = gtk4::Label::new(Some(crate::tr!("main-last-logins-title").as_str()));
        profile_title.set_halign(Align::Start);
        profile_title.add_css_class("profile-login-history-title");
        profile_box.append(&profile_title);

        let login_history_list = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .build();
        profile_box.append(&login_history_list);

        let runtime_for_login_history = runtime_handle.clone();
        let db_for_login_history = database_pool.clone();
        let history_list_for_show = login_history_list.clone();
        let runtime_for_login_history_click = runtime_handle.clone();
        let db_for_login_history_click = database_pool.clone();
        let history_list_for_click = login_history_list.clone();
        profile_popover.connect_show(move |_| {
            Self::refresh_login_history_popover(
                runtime_for_login_history.clone(),
                db_for_login_history.clone(),
                admin_user_id,
                history_list_for_show.clone(),
            );
        });
        profile_button.connect_notify_local(Some("active"), move |button, _| {
            if !button.property::<bool>("active") {
                return;
            }
            Self::refresh_login_history_popover(
                runtime_for_login_history_click.clone(),
                db_for_login_history_click.clone(),
                admin_user_id,
                history_list_for_click.clone(),
            );
        });

        profile_popover.set_child(Some(&profile_box));
        profile_button.set_popover(Some(&profile_popover));

        let user_identity_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();
        user_identity_box.append(&profile_button);
        if is_admin {
            let admin_badge = gtk4::Label::new(Some(crate::tr!("main-admin-badge").as_str()));
            admin_badge.add_css_class("header-badge");
            admin_badge.add_css_class("admin-badge");
            admin_badge.add_css_class("warning");
            admin_badge.set_tooltip_text(Some(crate::tr!("main-admin-badge-tooltip").as_str()));
            user_identity_box.append(&admin_badge);
        }

        let add_button = gtk4::Button::builder()
            .icon_name("list-add-symbolic")
            .build();
        add_button.add_css_class("flat");
        add_button.add_css_class("accent");
        add_button.add_css_class("main-add-button");
        add_button.set_tooltip_text(Some(crate::tr!("main-add-tooltip").as_str()));

        let trash_button = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .build();
        trash_button.add_css_class("flat");
        trash_button.add_css_class("main-add-button");
        trash_button.set_tooltip_text(Some(crate::tr!("main-trash-tooltip").as_str()));

        let certification_popover = gtk4::Popover::new();
        certification_popover.set_has_arrow(true);
        certification_popover.set_autohide(true);
        let certification_menu_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        certification_menu_box.add_css_class("profile-login-history-popover");
        let report_24h_menu_button =
            Self::build_certification_menu_item("document-save-symbolic", "Rapport 24h");
        let report_7d_menu_button =
            Self::build_certification_menu_item("document-save-symbolic", "Rapport 7 jours");
        let report_30d_menu_button =
            Self::build_certification_menu_item("document-save-symbolic", "Rapport 30 jours");
        let diagnostics_menu_button = Self::build_certification_menu_item(
            "emblem-system-symbolic",
            "Vérifier l'état de signature",
        );
        certification_menu_box.append(&report_24h_menu_button);
        certification_menu_box.append(&report_7d_menu_button);
        certification_menu_box.append(&report_30d_menu_button);
        certification_menu_box.append(&diagnostics_menu_button);
        certification_popover.set_child(Some(&certification_menu_box));

        let app_for_trash = application.clone();
        let window_for_trash = window.clone();
        let runtime_for_trash = runtime_handle.clone();
        let secret_for_trash = Arc::clone(&secret_service);
        let vault_for_trash = Arc::clone(&vault_service);
        let admin_user_for_trash = admin_user_id;
        let session_master_for_trash = Rc::clone(&session_master_key);
        let report_service_for_click = Arc::clone(&audit_report_service);
        let report_customer_for_click = report_customer_name.clone();
        let window_for_report = window.clone();
        let toast_overlay_for_report = toast_overlay.clone();
        let license_service_for_report = Arc::clone(&license_service);

        let center_panel = Self::build_center_panel();
        let sidebar_panel = Self::build_sidebar_panel();
        sidebar_panel
            .certification_menu_button
            .set_tooltip_text(Some("Certifier & Exporter"));
        sidebar_panel
            .certification_menu_button
            .set_popover(Some(&certification_popover));
        let certification_enabled = license_service_for_report
            .get_cached()
            .map(|license| matches!(license.tier, LicenseTier::Professional))
            .unwrap_or(false);
        sidebar_panel
            .certification_menu_button
            .set_sensitive(certification_enabled);
        if !certification_enabled {
            sidebar_panel
                .certification_menu_button
                .set_tooltip_text(Some("Certifier & Exporter (licence Pro requise)"));
        }
        let sidebar_i18n_refresh: Rc<dyn Fn()> = {
            let sidebar = SidebarWidgets {
                frame: sidebar_panel.frame.clone(),
                my_vaults_title: sidebar_panel.my_vaults_title.clone(),
                create_vault_button: sidebar_panel.create_vault_button.clone(),
                my_vaults_list: sidebar_panel.my_vaults_list.clone(),
                shared_vaults_title: sidebar_panel.shared_vaults_title.clone(),
                shared_vaults_list: sidebar_panel.shared_vaults_list.clone(),
                category_list: sidebar_panel.category_list.clone(),
                audit_list: sidebar_panel.audit_list.clone(),
                audit_title: sidebar_panel.audit_title.clone(),
                categories_title: sidebar_panel.categories_title.clone(),
                account_title: sidebar_panel.account_title.clone(),
                audit_all_label: sidebar_panel.audit_all_label.clone(),
                audit_weak_label: sidebar_panel.audit_weak_label.clone(),
                audit_duplicate_label: sidebar_panel.audit_duplicate_label.clone(),
                category_all_label: sidebar_panel.category_all_label.clone(),
                category_passwords_label: sidebar_panel.category_passwords_label.clone(),
                category_api_tokens_label: sidebar_panel.category_api_tokens_label.clone(),
                category_ssh_keys_label: sidebar_panel.category_ssh_keys_label.clone(),
                category_documents_label: sidebar_panel.category_documents_label.clone(),
                audit_all_badge: sidebar_panel.audit_all_badge.clone(),
                audit_weak_badge: sidebar_panel.audit_weak_badge.clone(),
                audit_duplicate_badge: sidebar_panel.audit_duplicate_badge.clone(),
                certification_menu_button: sidebar_panel.certification_menu_button.clone(),
                profile_security_label: sidebar_panel.profile_security_label.clone(),
                profile_security_button: sidebar_panel.profile_security_button.clone(),
                teams_label: sidebar_panel.teams_label.clone(),
                teams_button: sidebar_panel.teams_button.clone(),
                administration_label: sidebar_panel.administration_label.clone(),
                administration_button: sidebar_panel.administration_button.clone(),
            };
            Rc::new(move || {
                sidebar
                    .audit_title
                    .set_text(crate::tr!("main-audit-title").as_str());
                sidebar
                    .audit_all_label
                    .set_text(crate::tr!("main-audit-all").as_str());
                sidebar
                    .audit_weak_label
                    .set_text(crate::tr!("main-audit-weak").as_str());
                sidebar
                    .audit_duplicate_label
                    .set_text(crate::tr!("main-audit-duplicates").as_str());
                sidebar
                    .categories_title
                    .set_text(crate::tr!("main-categories-title").as_str());
                sidebar
                    .category_all_label
                    .set_text(crate::tr!("main-category-all").as_str());
                sidebar
                    .category_passwords_label
                    .set_text(crate::tr!("main-category-passwords").as_str());
                sidebar
                    .category_api_tokens_label
                    .set_text(crate::tr!("main-category-api-tokens").as_str());
                sidebar
                    .category_ssh_keys_label
                    .set_text(crate::tr!("main-category-ssh-keys").as_str());
                sidebar
                    .category_documents_label
                    .set_text(crate::tr!("main-category-documents").as_str());
                sidebar
                    .account_title
                    .set_text(crate::tr!("main-account-title").as_str());
                sidebar
                    .profile_security_label
                    .set_text(crate::tr!("main-profile-security").as_str());
                sidebar
                    .my_vaults_title
                    .set_text(crate::tr!("main-my-vaults-title").as_str());
                sidebar
                    .shared_vaults_title
                    .set_text(crate::tr!("main-shared-with-me-title").as_str());
                sidebar
                    .create_vault_button
                    .set_tooltip_text(Some(crate::tr!("main-create-vault-button").as_str()));
                sidebar
                    .teams_label
                    .set_text(crate::tr!("main-teams-nav").as_str());
                sidebar
                    .administration_label
                    .set_text(crate::tr!("main-user-nav").as_str());
                sidebar
                    .certification_menu_button
                    .set_tooltip_text(Some("Certifier & Exporter"));
            })
        };
        let on_main_i18n_refresh: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_main_i18n_refresh_bridge: Rc<dyn Fn()> = {
            let holder = Rc::clone(&on_main_i18n_refresh);
            Rc::new(move || {
                if let Some(refresh) = holder.borrow().as_ref() {
                    refresh();
                }
            })
        };
        let secret_flow_for_struct = center_panel.secret_flow.clone();
        let show_passwords_in_edit_pref = Rc::new(Cell::new(false));

        let filter_runtime = FilterRuntime {
            meta_by_widget: Rc::new(RefCell::new(HashMap::new())),
            search_text: Rc::new(RefCell::new(String::new())),
            selected_category: Rc::new(Cell::new(SecretCategoryFilter::All)),
            selected_audit: Rc::new(Cell::new(AuditFilter::All)),
            selected_sort: Rc::new(Cell::new(SecretSortMode::Recent)),
            audit_all_count_label: sidebar_panel.audit_all_badge.clone(),
            audit_weak_count_label: sidebar_panel.audit_weak_badge.clone(),
            audit_duplicate_count_label: sidebar_panel.audit_duplicate_badge.clone(),
            total_count_label: center_panel.status_total_badge.clone(),
            non_compliant_count_label: center_panel.status_non_compliant_badge.clone(),
            filtered_status_page: center_panel.filtered_status_page.clone(),
        };

        let runtime_for_flow_filter = filter_runtime.clone();
        center_panel.secret_flow.set_filter_func(move |child| {
            let Some(content) = child.child() else {
                return false;
            };
            let key = content.widget_name().to_string();
            let store = runtime_for_flow_filter.meta_by_widget.borrow();
            let Some(meta) = store.get(&key) else {
                return true;
            };

            let query = runtime_for_flow_filter.search_text.borrow().to_string();
            let terms = Self::parse_search_terms(query.as_str());
            let matches_query = terms.is_empty()
                || terms
                    .iter()
                    .all(|term| Self::matches_search_term(meta, term));

            let matches_category = match runtime_for_flow_filter.selected_category.get() {
                SecretCategoryFilter::All => true,
                SecretCategoryFilter::Password => meta.kind == SecretKind::Password,
                SecretCategoryFilter::ApiToken => meta.kind == SecretKind::ApiToken,
                SecretCategoryFilter::SshKey => meta.kind == SecretKind::SshKey,
                SecretCategoryFilter::SecureDocument => meta.kind == SecretKind::SecureDocument,
            };

            let matches_audit = match runtime_for_flow_filter.selected_audit.get() {
                AuditFilter::All => true,
                AuditFilter::Weak => meta.is_weak,
                AuditFilter::Duplicate => meta.is_duplicate,
            };

            matches_query && matches_category && matches_audit
        });

        let runtime_for_flow_sort = filter_runtime.clone();
        center_panel.secret_flow.set_sort_func(move |left, right| {
            let left_key = left
                .child()
                .map(|child| child.widget_name().to_string())
                .unwrap_or_default();
            let right_key = right
                .child()
                .map(|child| child.widget_name().to_string())
                .unwrap_or_default();

            let store = runtime_for_flow_sort.meta_by_widget.borrow();
            let Some(left_meta) = store.get(&left_key) else {
                return left_key.cmp(&right_key).into();
            };
            let Some(right_meta) = store.get(&right_key) else {
                return left_key.cmp(&right_key).into();
            };

            match runtime_for_flow_sort.selected_sort.get() {
                SecretSortMode::Recent => left_meta
                    .original_rank
                    .cmp(&right_meta.original_rank)
                    .into(),
                SecretSortMode::Title => left_meta
                    .title_text
                    .cmp(&right_meta.title_text)
                    .then(left_meta.original_rank.cmp(&right_meta.original_rank))
                    .into(),
                SecretSortMode::Risk => {
                    let left_score =
                        usize::from(left_meta.is_weak) + usize::from(left_meta.is_duplicate);
                    let right_score =
                        usize::from(right_meta.is_weak) + usize::from(right_meta.is_duplicate);
                    right_score
                        .cmp(&left_score)
                        .then(left_meta.title_text.cmp(&right_meta.title_text))
                        .then(left_meta.original_rank.cmp(&right_meta.original_rank))
                        .into()
                }
            }
        });

        let secret_flow_for_refresh = center_panel.secret_flow.clone();
        let stack_for_refresh = center_panel.stack.clone();
        let empty_title_for_refresh = center_panel.empty_title.clone();
        let empty_copy_for_refresh = center_panel.empty_copy.clone();
        let vault_selection_sync = Rc::new(Cell::new(false));
        let default_vault_creation_in_progress = Rc::new(Cell::new(false));
        let refresh_after_vault_mutation: Rc<RefCell<Option<Rc<dyn Fn()>>>> =
            Rc::new(RefCell::new(None));
        let editor_launcher: Rc<RefCell<Option<Rc<dyn Fn(DialogMode)>>>> =
            Rc::new(RefCell::new(None));

        let refresh_vault_sections: Rc<dyn Fn()> = {
            let app_for_secret_refresh = application.clone();
            let parent_for_secret_refresh = window.clone();
            let runtime_for_secret_refresh = runtime_handle.clone();
            let secret_service_for_secret_refresh = Arc::clone(&secret_service);
            let vault_service_for_secret_refresh = Arc::clone(&vault_service);
            let toast_for_secret_refresh = toast_overlay.clone();
            let active_vault_for_secret_refresh = Rc::clone(&active_vault_id);
            let secret_flow_for_secret_refresh = secret_flow_for_refresh.clone();
            let stack_for_secret_refresh = stack_for_refresh.clone();
            let empty_title_for_secret_refresh = empty_title_for_refresh.clone();
            let empty_copy_for_secret_refresh = empty_copy_for_refresh.clone();
            let filter_for_secret_refresh = filter_runtime.clone();
            let editor_launcher_for_secret_refresh = Rc::clone(&editor_launcher);
            let session_for_secret_refresh = Rc::clone(&session_master_key);
            let refresh_secrets: Rc<dyn Fn()> = Rc::new(move || {
                let Some(master_key) =
                    Self::snapshot_session_master_key(&session_for_secret_refresh)
                else {
                    empty_title_for_secret_refresh
                        .set_text(crate::tr!("main-session-locked-title").as_str());
                    empty_copy_for_secret_refresh
                        .set_text(crate::tr!("main-session-locked-description").as_str());
                    stack_for_secret_refresh.set_visible_child_name("empty");
                    return;
                };

                Self::refresh_secret_flow(
                    app_for_secret_refresh.clone(),
                    parent_for_secret_refresh.clone(),
                    runtime_for_secret_refresh.clone(),
                    Arc::clone(&secret_service_for_secret_refresh),
                    Arc::clone(&vault_service_for_secret_refresh),
                    admin_user_id,
                    master_key,
                    secret_flow_for_secret_refresh.clone(),
                    stack_for_secret_refresh.clone(),
                    empty_title_for_secret_refresh.clone(),
                    empty_copy_for_secret_refresh.clone(),
                    active_vault_for_secret_refresh.clone(),
                    toast_for_secret_refresh.clone(),
                    filter_for_secret_refresh.clone(),
                    editor_launcher_for_secret_refresh.clone(),
                );
            });

            let runtime_for_vaults = runtime_handle.clone();
            let vault_service_for_vaults = Arc::clone(&vault_service);
            let secret_service_for_vaults = Arc::clone(&secret_service);
            let session_master_for_vaults = Rc::clone(&session_master_key);
            let my_vaults_list = sidebar_panel.my_vaults_list.clone();
            let shared_vaults_title = sidebar_panel.shared_vaults_title.clone();
            let shared_vaults_list = sidebar_panel.shared_vaults_list.clone();
            let active_vault_id = Rc::clone(&active_vault_id);
            let vault_selection_sync = Rc::clone(&vault_selection_sync);
            let creating_default_vault = Rc::clone(&default_vault_creation_in_progress);
            let refresh_secrets_for_vaults = Rc::clone(&refresh_secrets);
            let window_for_delete_vault = window.clone();
            let runtime_for_delete_vault = runtime_handle.clone();
            let vault_service_for_delete_vault = Arc::clone(&vault_service);
            let refresh_after_vault_mutation = Rc::clone(&refresh_after_vault_mutation);
            Rc::new(move || {
                let refresh_after_delete_holder = Rc::clone(&refresh_after_vault_mutation);
                let active_vault_for_delete = Rc::clone(&active_vault_id);
                let delete_vault_action: Rc<dyn Fn(Uuid, String)> = {
                    let parent_window = window_for_delete_vault.clone();
                    let runtime_for_action = runtime_for_delete_vault.clone();
                    let vault_service_for_action = Arc::clone(&vault_service_for_delete_vault);
                    Rc::new(move |vault_id: Uuid, vault_name: String| {
                        let body = crate::i18n::tr_args(
                            "main-delete-vault-confirm-body",
                            &[("name", crate::i18n::I18nArg::Str(vault_name.as_str()))],
                        );
                        let dialog = adw::MessageDialog::new(
                            Some(&parent_window),
                            Some(crate::tr!("main-delete-vault-confirm-title").as_str()),
                            Some(body.as_str()),
                        );
                        dialog.add_response("cancel", crate::tr!("common-cancel").as_str());
                        dialog.add_response(
                            "delete",
                            crate::tr!("main-delete-vault-confirm-cta").as_str(),
                        );
                        dialog.set_response_appearance(
                            "delete",
                            adw::ResponseAppearance::Destructive,
                        );
                        dialog.set_default_response(Some("cancel"));
                        dialog.set_close_response("cancel");

                        let runtime_for_confirm = runtime_for_action.clone();
                        let vault_service_for_confirm = Arc::clone(&vault_service_for_action);
                        let parent_for_confirm = parent_window.clone();
                        let refresh_after_confirm_holder = Rc::clone(&refresh_after_delete_holder);
                        let active_for_confirm = Rc::clone(&active_vault_for_delete);
                        dialog.connect_response(None, move |_dlg, response| {
                            if response != "delete" {
                                return;
                            }

                            let (sender, receiver) = tokio::sync::oneshot::channel();
                            let runtime_for_task = runtime_for_confirm.clone();
                            let vault_service_for_task = Arc::clone(&vault_service_for_confirm);
                            std::thread::spawn(move || {
                                let result = runtime_for_task.block_on(async move {
                                    vault_service_for_task
                                        .delete_vault(admin_user_id, vault_id)
                                        .await
                                });
                                let _ = sender.send(result);
                            });

                            let parent_for_result = parent_for_confirm.clone();
                            let refresh_after_result_holder =
                                Rc::clone(&refresh_after_confirm_holder);
                            let active_after_result = Rc::clone(&active_for_confirm);
                            glib::MainContext::default().spawn_local(async move {
                                match receiver.await {
                                    Ok(Ok(())) => {
                                        if *active_after_result.borrow() == Some(vault_id) {
                                            *active_after_result.borrow_mut() = None;
                                        }
                                        if let Some(refresh) =
                                            refresh_after_result_holder.borrow().as_ref()
                                        {
                                            refresh();
                                        }
                                    }
                                    Ok(Err(error)) => {
                                        let message = error.to_string();
                                        Self::show_feedback_dialog(
                                            &parent_for_result,
                                            crate::tr!("main-delete-vault-error-title").as_str(),
                                            message.as_str(),
                                        );
                                    }
                                    Err(_) => {
                                        Self::show_feedback_dialog(
                                            &parent_for_result,
                                            crate::tr!("main-delete-vault-error-title").as_str(),
                                            crate::tr!("main-list-unavailable-description")
                                                .as_str(),
                                        );
                                    }
                                }
                            });
                        });
                        dialog.present();
                    })
                };

                while let Some(child) = my_vaults_list.first_child() {
                    my_vaults_list.remove(&child);
                }
                while let Some(child) = shared_vaults_list.first_child() {
                    shared_vaults_list.remove(&child);
                }

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let runtime_for_task = runtime_for_vaults.clone();
                let vault_service_for_task = Arc::clone(&vault_service_for_vaults);
                let secret_service_for_task = Arc::clone(&secret_service_for_vaults);
                let can_attempt_default_create = !creating_default_vault.get();
                let master_for_default_create = if can_attempt_default_create {
                    Self::snapshot_session_master_key(&session_master_for_vaults)
                } else {
                    None
                };
                if can_attempt_default_create && master_for_default_create.is_some() {
                    creating_default_vault.set(true);
                }
                std::thread::spawn(move || {
                    let result = runtime_for_task.block_on(async move {
                        let mut owned_vaults = vault_service_for_task
                            .list_owned_vaults(admin_user_id)
                            .await?;
                        if owned_vaults.is_empty() {
                            if let Some(master_key) = master_for_default_create {
                                let _ = vault_service_for_task
                                    .create_vault(
                                        admin_user_id,
                                        "perso",
                                        SecretBox::new(Box::new(master_key)),
                                    )
                                    .await;
                                owned_vaults = vault_service_for_task
                                    .list_owned_vaults(admin_user_id)
                                    .await?;
                            }
                        }
                        let mut owned_with_counts: Vec<(crate::models::Vault, usize, bool)> =
                            Vec::with_capacity(owned_vaults.len());
                        for vault in owned_vaults {
                            let count =
                                secret_service_for_task.list_by_vault(vault.id).await?.len();
                            let is_shared_with_others = vault_service_for_task
                                .is_vault_shared_with_others(admin_user_id, vault.id)
                                .await?;
                            owned_with_counts.push((vault, count, is_shared_with_others));
                        }

                        let shared_access = vault_service_for_task
                            .list_shared_vault_access(admin_user_id)
                            .await?;
                        let mut shared_with_counts: Vec<(crate::models::AccessibleVault, usize)> =
                            Vec::with_capacity(shared_access.len());
                        for access in shared_access {
                            let count = secret_service_for_task
                                .list_by_vault(access.vault.id)
                                .await?
                                .len();
                            shared_with_counts.push((access, count));
                        }

                        Ok::<_, crate::errors::AppError>((owned_with_counts, shared_with_counts))
                    });
                    let _ = sender.send((result, can_attempt_default_create));
                });
                let my_vaults_for_recv = my_vaults_list.clone();
                let shared_title_for_recv = shared_vaults_title.clone();
                let shared_list_for_recv = shared_vaults_list.clone();
                let active_vault_for_recv = Rc::clone(&active_vault_id);
                let sync_for_recv = Rc::clone(&vault_selection_sync);
                let creating_default_vault_for_recv = Rc::clone(&creating_default_vault);
                let delete_action_for_recv = Rc::clone(&delete_vault_action);
                let refresh_secrets_for_recv = Rc::clone(&refresh_secrets_for_vaults);
                glib::MainContext::default().spawn_local(async move {
                    match receiver.await {
                        Ok((Ok((owned_vaults, shared_vaults)), attempted_default_create)) => {
                            if attempted_default_create {
                                creating_default_vault_for_recv.set(false);
                            }
                            sync_for_recv.set(true);
                            let mut first_vault_id: Option<Uuid> = None;
                            for (vault, secret_count, is_shared_with_others) in owned_vaults {
                                if first_vault_id.is_none() {
                                    first_vault_id = Some(vault.id);
                                }
                                let row = Self::build_vault_sidebar_row(
                                    vault.name.as_str(),
                                    vault.id,
                                    true,
                                    is_shared_with_others,
                                    None,
                                    secret_count,
                                    Some(Rc::clone(&delete_action_for_recv)),
                                );
                                my_vaults_for_recv.append(&row);
                            }

                            let mut first_shared_vault_id: Option<Uuid> = None;
                            for (access, secret_count) in shared_vaults {
                                if first_shared_vault_id.is_none() {
                                    first_shared_vault_id = Some(access.vault.id);
                                }
                                let row = Self::build_vault_sidebar_row(
                                    access.vault.name.as_str(),
                                    access.vault.id,
                                    false,
                                    false,
                                    Some(access.role),
                                    secret_count,
                                    None,
                                );
                                shared_list_for_recv.append(&row);
                            }
                            let has_shared = shared_list_for_recv.first_child().is_some();
                            shared_title_for_recv.set_visible(has_shared);
                            shared_list_for_recv.set_visible(has_shared);

                            let active = *active_vault_for_recv.borrow();
                            if let Some(active_id) = active {
                                if let Some(row) =
                                    Self::find_vault_row(&my_vaults_for_recv, active_id)
                                {
                                    my_vaults_for_recv.select_row(Some(&row));
                                } else if let Some(row) =
                                    Self::find_vault_row(&shared_list_for_recv, active_id)
                                {
                                    shared_list_for_recv.select_row(Some(&row));
                                } else if let Some(fallback_id) = first_vault_id {
                                    *active_vault_for_recv.borrow_mut() = Some(fallback_id);
                                    if let Some(row) =
                                        Self::find_vault_row(&my_vaults_for_recv, fallback_id)
                                    {
                                        my_vaults_for_recv.select_row(Some(&row));
                                    }
                                } else if let Some(fallback_id) = first_shared_vault_id {
                                    *active_vault_for_recv.borrow_mut() = Some(fallback_id);
                                    if let Some(row) =
                                        Self::find_vault_row(&shared_list_for_recv, fallback_id)
                                    {
                                        shared_list_for_recv.select_row(Some(&row));
                                    }
                                } else {
                                    *active_vault_for_recv.borrow_mut() = None;
                                    my_vaults_for_recv.unselect_all();
                                    shared_list_for_recv.unselect_all();
                                }
                            } else if let Some(fallback_id) = first_vault_id {
                                *active_vault_for_recv.borrow_mut() = Some(fallback_id);
                                if let Some(row) =
                                    Self::find_vault_row(&my_vaults_for_recv, fallback_id)
                                {
                                    my_vaults_for_recv.select_row(Some(&row));
                                }
                            } else if let Some(fallback_id) = first_shared_vault_id {
                                *active_vault_for_recv.borrow_mut() = Some(fallback_id);
                                if let Some(row) =
                                    Self::find_vault_row(&shared_list_for_recv, fallback_id)
                                {
                                    shared_list_for_recv.select_row(Some(&row));
                                }
                            }
                            sync_for_recv.set(false);
                            refresh_secrets_for_recv();
                        }
                        Ok((Err(_), attempted_default_create)) => {
                            if attempted_default_create {
                                creating_default_vault_for_recv.set(false);
                            }
                            sync_for_recv.set(false);
                        }
                        _ => {
                            sync_for_recv.set(false);
                        }
                    }
                });
            })
        };

        let refresh_list: Rc<dyn Fn()> = {
            let refresh_vault_sections = Rc::clone(&refresh_vault_sections);
            Rc::new(move || {
                refresh_vault_sections();
            })
        };
        *refresh_after_vault_mutation.borrow_mut() = Some(Rc::clone(&refresh_list));

        let window_for_create_vault = window.clone();
        let runtime_for_create_vault = runtime_handle.clone();
        let vault_service_for_create_vault = Arc::clone(&vault_service);
        let session_for_create_vault = Rc::clone(&session_master_key);
        let refresh_for_create_vault = Rc::clone(&refresh_list);
        sidebar_panel.create_vault_button.connect_clicked(move |_| {
            let create_window = gtk4::Window::builder()
                .title(crate::tr!("main-create-vault-window-title").as_str())
                .modal(true)
                .transient_for(&window_for_create_vault)
                .default_width(440)
                .default_height(140)
                .build();

            let root = gtk4::Box::builder()
                .orientation(Orientation::Vertical)
                .spacing(10)
                .margin_top(12)
                .margin_bottom(12)
                .margin_start(12)
                .margin_end(12)
                .build();
            let name_entry = gtk4::Entry::new();
            name_entry.set_placeholder_text(Some(
                crate::tr!("main-create-vault-name-placeholder").as_str(),
            ));

            let actions = gtk4::Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(8)
                .halign(Align::End)
                .build();
            let cancel = gtk4::Button::with_label(crate::tr!("common-cancel").as_str());
            let create = gtk4::Button::with_label(crate::tr!("main-create-vault-create").as_str());
            create.add_css_class("suggested-action");
            actions.append(&cancel);
            actions.append(&create);

            root.append(&name_entry);
            root.append(&actions);
            create_window.set_child(Some(&root));

            let create_window_for_cancel = create_window.clone();
            cancel.connect_clicked(move |_| create_window_for_cancel.close());

            let runtime_for_apply = runtime_for_create_vault.clone();
            let vault_for_apply = Arc::clone(&vault_service_for_create_vault);
            let session_for_apply = Rc::clone(&session_for_create_vault);
            let window_for_apply = window_for_create_vault.clone();
            let refresh_after_apply = Rc::clone(&refresh_for_create_vault);
            let create_window_for_apply = create_window.clone();
            create.connect_clicked(move |_| {
                let vault_name = name_entry.text().to_string();
                if vault_name.trim().is_empty() {
                    Self::show_feedback_dialog(
                        &window_for_apply,
                        crate::tr!("main-create-vault-window-title").as_str(),
                        crate::tr!("main-create-vault-error-empty-name").as_str(),
                    );
                    return;
                }

                let Some(master_key) = Self::snapshot_session_master_key(&session_for_apply) else {
                    Self::show_feedback_dialog(
                        &window_for_apply,
                        crate::tr!("main-create-vault-window-title").as_str(),
                        crate::tr!("main-create-vault-error-session-locked").as_str(),
                    );
                    return;
                };

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let runtime_for_task = runtime_for_apply.clone();
                let vault_for_task = Arc::clone(&vault_for_apply);
                std::thread::spawn(move || {
                    let result = runtime_for_task.block_on(async move {
                        vault_for_task
                            .create_vault(
                                admin_user_id,
                                vault_name.trim(),
                                SecretBox::new(Box::new(master_key)),
                            )
                            .await
                    });
                    let _ = sender.send(result);
                });

                let window_for_result = window_for_apply.clone();
                let refresh_after_result = Rc::clone(&refresh_after_apply);
                let create_window_close = create_window_for_apply.clone();
                glib::MainContext::default().spawn_local(async move {
                    match receiver.await {
                        Ok(Ok(_)) => {
                            create_window_close.close();
                            refresh_after_result();
                        }
                        Ok(Err(error)) => {
                            let message = error.to_string();
                            Self::show_feedback_dialog(
                                &window_for_result,
                                crate::tr!("main-create-vault-window-title").as_str(),
                                message.as_str(),
                            );
                        }
                        Err(_) => {
                            Self::show_feedback_dialog(
                                &window_for_result,
                                crate::tr!("main-create-vault-window-title").as_str(),
                                crate::tr!("main-list-unavailable-description").as_str(),
                            );
                        }
                    }
                });
            });

            create_window.present();
        });

        let profile_view = Self::build_profile_view(
            window.clone(),
            runtime_handle.clone(),
            Arc::clone(&user_service),
            Arc::clone(&totp_service),
            Arc::clone(&auth_policy_service),
            Arc::clone(&backup_service),
            Arc::clone(&backup_app_service),
            Arc::clone(&import_service),
            Arc::clone(&secret_service),
            Arc::clone(&vault_service),
            database_path.clone(),
            admin_user_id,
            is_admin,
            profile_button.clone(),
            Rc::clone(&critical_ops_in_flight),
            Rc::clone(&auto_lock_timeout_secs),
            Rc::clone(&auto_lock_source),
            Rc::clone(&auto_lock_armed),
            Rc::clone(&on_auto_lock),
            Rc::clone(&session_master_key),
            Rc::clone(&show_passwords_in_edit_pref),
            Rc::clone(&refresh_list),
            Rc::clone(&on_main_i18n_refresh_bridge),
        );
        center_panel.main_stack.add_titled(
            &profile_view.container,
            Some("profile_view"),
            crate::tr!("main-profile-security").as_str(),
        );

        let main_stack_for_back = center_panel.main_stack.clone();
        profile_view.back_button.connect_clicked(move |_| {
            main_stack_for_back.set_visible_child_name("entries_view");
        });

        let main_stack_for_profile = center_panel.main_stack.clone();
        sidebar_panel
            .profile_security_button
            .connect_clicked(move |_| {
                main_stack_for_profile.set_visible_child_name("profile_view");
            });

        let users_view_container = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        let users_view_root = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(18)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        users_view_root.add_css_class("profile-view-content");
        let users_header = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .build();
        users_header.add_css_class("profile-view-header");
        let users_back_button = gtk4::Button::builder()
            .label(crate::tr!("main-view-back").as_str())
            .icon_name("go-previous-symbolic")
            .build();
        users_back_button.add_css_class("flat");
        users_back_button.add_css_class("main-inline-back-button");
        users_back_button.set_halign(Align::Start);
        let users_title = gtk4::Label::new(Some(crate::tr!("main-users-view-title").as_str()));
        users_title.add_css_class("title-3");
        users_title.add_css_class("heading");
        users_title.add_css_class("main-section-title");
        users_title.set_hexpand(true);
        users_title.set_halign(Align::Center);
        users_header.append(&users_back_button);
        users_header.append(&users_title);
        users_view_root.append(&users_header);
        let users_intro = gtk4::Label::new(Some(crate::tr!("main-users-view-intro").as_str()));
        users_intro.set_halign(Align::Start);
        users_intro.set_wrap(true);
        users_intro.add_css_class("dim-label");
        users_view_root.append(&users_intro);
        if is_admin {
            let app_for_admin = application.clone();
            let window_for_admin = window.clone();
            let runtime_for_admin = runtime_handle.clone();
            let admin_for_admin = Arc::clone(&admin_service);
            let users_dialog = ManageUsersDialog::new(
                &app_for_admin,
                &window_for_admin,
                runtime_for_admin,
                Arc::clone(&admin_for_admin),
                Arc::clone(&vault_service),
                admin_user_id,
            );
            let users_content = users_dialog.take_content().unwrap_or_else(|| {
                gtk4::Box::new(Orientation::Vertical, 0).upcast::<gtk4::Widget>()
            });
            let users_frame = gtk4::Frame::new(None);
            users_frame.add_css_class("profile-section-frame");
            users_frame.set_child(Some(&users_content));
            users_view_root.append(&users_frame);
            users_view_container.set_child(Some(&users_view_root));
            center_panel.main_stack.add_titled(
                &users_view_container,
                Some("users_view"),
                crate::tr!("main-user-nav").as_str(),
            );
            let main_stack_for_users_back = center_panel.main_stack.clone();
            users_back_button.connect_clicked(move |_| {
                main_stack_for_users_back.set_visible_child_name("entries_view");
            });
            let main_stack_for_users = center_panel.main_stack.clone();
            sidebar_panel
                .administration_button
                .connect_clicked(move |_| {
                    main_stack_for_users.set_visible_child_name("users_view");
                });
        }

        let teams_view_container = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        let teams_view_root = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(18)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        teams_view_root.add_css_class("profile-view-content");
        let teams_header = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .build();
        teams_header.add_css_class("profile-view-header");
        let teams_back_button = gtk4::Button::builder()
            .label(crate::tr!("main-view-back").as_str())
            .icon_name("go-previous-symbolic")
            .build();
        teams_back_button.add_css_class("flat");
        teams_back_button.add_css_class("main-inline-back-button");
        teams_back_button.set_halign(Align::Start);
        let teams_title = gtk4::Label::new(Some(crate::tr!("main-teams-view-title").as_str()));
        teams_title.add_css_class("title-3");
        teams_title.add_css_class("heading");
        teams_title.add_css_class("main-section-title");
        teams_title.set_hexpand(true);
        teams_title.set_halign(Align::Center);
        teams_header.append(&teams_back_button);
        teams_header.append(&teams_title);
        teams_view_root.append(&teams_header);
        let teams_intro = gtk4::Label::new(Some(crate::tr!("main-teams-view-intro").as_str()));
        teams_intro.set_halign(Align::Start);
        teams_intro.set_wrap(true);
        teams_intro.add_css_class("dim-label");
        teams_view_root.append(&teams_intro);
        if is_admin {
            let app_for_teams = application.clone();
            let window_for_teams = window.clone();
            let runtime_for_teams = runtime_handle.clone();
            let team_for_teams = Arc::clone(&team_service);
            let vault_for_teams = Arc::clone(&vault_service);
            let master_for_teams = Rc::clone(&session_master_key);
            let active_vault_for_teams = Rc::clone(&active_vault_id);
            let refresh_for_teams = Rc::clone(&refresh_list);
            let teams_dialog = ManageTeamsDialog::new(
                &app_for_teams,
                &window_for_teams,
                runtime_for_teams,
                Arc::clone(&team_for_teams),
                Arc::clone(&vault_for_teams),
                admin_user_id,
                Rc::clone(&master_for_teams),
                Rc::clone(&active_vault_for_teams),
                Rc::clone(&refresh_for_teams),
            );
            let teams_content = teams_dialog.take_content().unwrap_or_else(|| {
                gtk4::Box::new(Orientation::Vertical, 0).upcast::<gtk4::Widget>()
            });
            let teams_frame = gtk4::Frame::new(None);
            teams_frame.add_css_class("profile-section-frame");
            teams_frame.set_child(Some(&teams_content));
            teams_view_root.append(&teams_frame);
            teams_view_container.set_child(Some(&teams_view_root));
            center_panel.main_stack.add_titled(
                &teams_view_container,
                Some("teams_view"),
                crate::tr!("main-teams-nav").as_str(),
            );
            let main_stack_for_teams_back = center_panel.main_stack.clone();
            teams_back_button.connect_clicked(move |_| {
                main_stack_for_teams_back.set_visible_child_name("entries_view");
            });
            let main_stack_for_teams = center_panel.main_stack.clone();
            sidebar_panel.teams_button.connect_clicked(move |_| {
                main_stack_for_teams.set_visible_child_name("teams_view");
            });
        }

        sidebar_panel.administration_button.set_visible(is_admin);
        sidebar_panel.teams_button.set_visible(is_admin);

        let secret_editor_host = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .vexpand(true)
            .hexpand(true)
            .build();
        center_panel.main_stack.add_titled(
            &secret_editor_host,
            Some("secret_editor_view"),
            crate::tr!("main-stack-editor").as_str(),
        );

        let open_editor: Rc<dyn Fn(DialogMode)> = {
            let runtime_for_editor = runtime_handle.clone();
            let secret_for_editor = Arc::clone(&secret_service);
            let vault_for_editor = Arc::clone(&vault_service);
            let session_for_editor = Rc::clone(&session_master_key);
            let refresh_after_save = Rc::clone(&refresh_list);
            let toast_overlay_for_editor = toast_overlay.clone();
            let show_passwords_in_edit_pref = Rc::clone(&show_passwords_in_edit_pref);
            let stack_for_editor = center_panel.main_stack.clone();
            let host_for_editor = secret_editor_host.clone();
            Rc::new(move |mode| {
                let Some(master_key) = Self::snapshot_session_master_key(&session_for_editor)
                else {
                    info!("secret editor blocked: session is locked");
                    return;
                };

                while let Some(child) = host_for_editor.first_child() {
                    host_for_editor.remove(&child);
                }

                let stack_for_cancel = stack_for_editor.clone();
                let inline_view = AddEditDialog::build_inline(
                    runtime_for_editor.clone(),
                    Arc::clone(&secret_for_editor),
                    Arc::clone(&vault_for_editor),
                    admin_user_id,
                    master_key,
                    show_passwords_in_edit_pref.get(),
                    mode,
                    move || {
                        stack_for_cancel.set_visible_child_name("entries_view");
                    },
                    {
                        let refresh_after_save = Rc::clone(&refresh_after_save);
                        let toast_overlay_for_save = toast_overlay_for_editor.clone();
                        move |saved_title: String| {
                            refresh_after_save();
                            let toast_message = messages::toast_secret_saved(saved_title.as_str());
                            toast_overlay_for_save
                                .add_toast(adw::Toast::new(toast_message.as_str()));
                        }
                    },
                );

                host_for_editor.append(&inline_view.container);
                stack_for_editor.set_visible_child_name("secret_editor_view");
            })
        };
        *editor_launcher.borrow_mut() = Some(open_editor.clone());

        // ── Bouton Urgence (Panic) ─────────────────────────────────────────
        // Place dans la zone de fin de la HeaderBar pour être visible mais
        // séparé des actions habituelles.
        let panic_button = gtk4::Button::new();
        panic_button.add_css_class("panic-badge");
        panic_button.set_tooltip_text(Some(crate::tr!("main-panic-tooltip").as_str()));

        let panic_inner = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(5)
            .valign(Align::Center)
            .build();
        let panic_icon = gtk4::Image::from_icon_name("system-shutdown-symbolic");
        panic_icon.add_css_class("panic-badge-label");
        let panic_lbl = gtk4::Label::new(Some(crate::tr!("main-panic-label").as_str()));
        panic_lbl.add_css_class("panic-badge-label");
        panic_inner.append(&panic_icon);
        panic_inner.append(&panic_lbl);
        panic_button.set_child(Some(&panic_inner));

        let window_for_panic = window.clone();
        panic_button.connect_clicked(move |_| {
            let dialog = adw::MessageDialog::new(
                Some(&window_for_panic),
                Some(crate::tr!("main-panic-title").as_str()),
                Some(crate::tr!("main-panic-body").as_str()),
            );
            dialog.add_response("cancel", crate::tr!("main-panic-cancel").as_str());
            dialog.add_response("wipe_exit", crate::tr!("main-panic-confirm").as_str());
            dialog.set_response_appearance("wipe_exit", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            dialog.connect_response(None, |_dlg, response| {
                if response == "wipe_exit" {
                    // Journalise l'événement avant la sortie.
                    // DailyLogFileWriter est synchrone : le message est écrit
                    // sur disque avant que process::exit ne termine le processus.
                    info!("Panic mode activated - wiping memory and exiting");
                    // Les SecretBox allouées dans les closures GTK seront
                    // retirées de la mémoire virtuelle par le noyau lors de la
                    // libération du tas du processus. L'OS zero-fill les pages
                    // avant de les réattribuer (comportement garanti par Linux).
                    std::process::exit(0);
                }
            });
            dialog.present();
        });

        let open_editor_for_add = open_editor.clone();
        let active_vault_for_add = Rc::clone(&active_vault_id);
        let runtime_for_add = runtime_handle.clone();
        let vault_service_for_add = Arc::clone(&vault_service);
        let window_for_add = window.clone();
        add_button.connect_clicked(move |_| {
            let maybe_vault_id = *active_vault_for_add.borrow();
            let Some(vault_id) = maybe_vault_id else {
                open_editor_for_add(DialogMode::Create);
                return;
            };

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let runtime_for_task = runtime_for_add.clone();
            let vault_for_task = Arc::clone(&vault_service_for_add);
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    let access = vault_for_task
                        .get_vault_access_for_user(admin_user_id, vault_id)
                        .await?
                        .ok_or({
                            crate::errors::AppError::Authorization(
                                crate::errors::AccessDeniedReason::VaultAccessDenied,
                            )
                        })?;
                    let is_shared = access.vault.owner_user_id != admin_user_id;
                    Ok::<bool, crate::errors::AppError>(!is_shared || access.role.can_admin())
                });
                let _ = sender.send(result);
            });

            let open_editor_for_result = open_editor_for_add.clone();
            let window_for_result = window_for_add.clone();
            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(true)) => {
                        open_editor_for_result(DialogMode::CreateInVault(vault_id));
                    }
                    Ok(Ok(false)) => {
                        Self::show_feedback_dialog(
                            &window_for_result,
                            crate::tr!("main-add-shared-denied-title").as_str(),
                            crate::tr!("main-add-shared-denied-body").as_str(),
                        );
                    }
                    _ => {
                        Self::show_feedback_dialog(
                            &window_for_result,
                            crate::tr!("main-add-shared-denied-title").as_str(),
                            crate::tr!("main-list-unavailable-description").as_str(),
                        );
                    }
                }
            });
        });
        let refresh_for_trash = Rc::clone(&refresh_list);
        header_bar.pack_start(&add_button);
        trash_button.connect_clicked(move |_| {
            let Some(master_key) = Self::snapshot_session_master_key(&session_master_for_trash)
            else {
                info!("trash access blocked: session is locked");
                return;
            };
            let refresh_after_trash = Rc::clone(&refresh_for_trash);
            let dialog = TrashDialog::new(
                &app_for_trash,
                &window_for_trash,
                runtime_for_trash.clone(),
                Arc::clone(&secret_for_trash),
                Arc::clone(&vault_for_trash),
                admin_user_for_trash,
                master_key,
                move || {
                    refresh_after_trash();
                },
            );
            dialog.present();
        });
        header_bar.pack_start(&trash_button);
        let launch_signed_report: Rc<dyn Fn(i64)> = Rc::new({
            let report_service = Arc::clone(&report_service_for_click);
            let customer_name = report_customer_for_click.clone();
            let report_window = window_for_report.clone();
            let report_toast_overlay = toast_overlay_for_report.clone();
            move |days| {
                report_toast_overlay
                    .add_toast(adw::Toast::new("Génération du rapport signé en cours..."));

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let report_service_for_task = Arc::clone(&report_service);
                let customer_for_task = customer_name.clone();
                std::thread::spawn(move || {
                    let result = report_service_for_task
                        .generate_audit_report(customer_for_task.as_str(), days);
                    let _ = sender.send(result);
                });

                let window_for_result = report_window.clone();
                let toast_overlay_for_result = report_toast_overlay.clone();
                glib::MainContext::default().spawn_local(async move {
                    match receiver.await {
                        Ok(Ok(report)) => {
                            toast_overlay_for_result.add_toast(adw::Toast::new(
                                format!(
                                    "Rapport certifié généré (SHA-256: {})",
                                    report.hash_prefix()
                                )
                                .as_str(),
                            ));
                            Self::show_feedback_dialog(
                                &window_for_result,
                                "Rapport signé généré",
                                format!("Rapport PDF signé généré avec succès:\n{}", report.path)
                                    .as_str(),
                            );
                        }
                        Ok(Err(ReportError::LicenseRequired)) => {
                            toast_overlay_for_result.add_toast(adw::Toast::new(
                                "Le rapport signé nécessite une licence Pro.",
                            ));
                        }
                        Ok(Err(ReportError::SigningKeyMissing)) => {
                            toast_overlay_for_result.add_toast(adw::Toast::new(
								"Clé de certification indisponible. Ouvrez la Console de Confiance.",
							));
                        }
                        Ok(Err(error)) => Self::show_feedback_dialog(
                            &window_for_result,
                            "Erreur de génération",
                            format!("Impossible de générer le rapport: {}", error).as_str(),
                        ),
                        Err(_) => Self::show_feedback_dialog(
                            &window_for_result,
                            "Erreur de génération",
                            "La génération du rapport a été interrompue.",
                        ),
                    }
                });
            }
        });
        report_24h_menu_button.connect_clicked({
            let launch_signed_report = Rc::clone(&launch_signed_report);
            let certification_popover = certification_popover.clone();
            move |_| {
                certification_popover.popdown();
                launch_signed_report(1)
            }
        });
        report_7d_menu_button.connect_clicked({
            let launch_signed_report = Rc::clone(&launch_signed_report);
            let certification_popover = certification_popover.clone();
            move |_| {
                certification_popover.popdown();
                launch_signed_report(7)
            }
        });
        report_30d_menu_button.connect_clicked({
            let launch_signed_report = Rc::clone(&launch_signed_report);
            let certification_popover = certification_popover.clone();
            move |_| {
                certification_popover.popdown();
                launch_signed_report(30)
            }
        });
        diagnostics_menu_button.connect_clicked({
            let certification_popover = certification_popover.clone();
            let report_window = window.clone();
            let license_service = Arc::clone(&license_service_for_report);
            move |_| {
                certification_popover.popdown();
                Self::show_certification_diagnostics_dialog(
                    &report_window,
                    Arc::clone(&license_service),
                );
            }
        });
        header_bar.pack_end(&user_identity_box);
        header_bar.pack_end(&panic_button);
        root.append(&header_bar);

        let (content, search_entry) =
            shell::build_content_shell(&sidebar_panel.frame, &center_panel.frame);
        root.append(&content);
        toast_overlay.set_child(Some(&root));
        window.set_content(Some(&toast_overlay));

        let main_i18n_refresh: Rc<dyn Fn()> = {
            let sidebar_i18n_refresh = Rc::clone(&sidebar_i18n_refresh);
            let profile_button = profile_button.clone();
            let profile_title = profile_title.clone();
            let add_button = add_button.clone();
            let trash_button = trash_button.clone();
            let panic_button = panic_button.clone();
            let panic_lbl = panic_lbl.clone();
            let search_entry = search_entry.clone();
            let status_total_chip = center_panel.status_total_chip.clone();
            let status_non_compliant_chip = center_panel.status_non_compliant_chip.clone();
            let sort_recent_button = center_panel.sort_recent_button.clone();
            let sort_title_button = center_panel.sort_title_button.clone();
            let sort_risk_button = center_panel.sort_risk_button.clone();
            let filtered_status_page = center_panel.filtered_status_page.clone();
            let entries_stack = center_panel.stack.clone();
            let list_page = center_panel.list_page.clone();
            let empty_state = center_panel.empty_state.clone();
            let main_stack = center_panel.main_stack.clone();
            let profile_container = profile_view.container.clone();
            let secret_editor_host = secret_editor_host.clone();
            let users_view_container = users_view_container.clone();
            let teams_view_container = teams_view_container.clone();
            let users_back_button = users_back_button.clone();
            let users_title = users_title.clone();
            let users_intro = users_intro.clone();
            let teams_back_button = teams_back_button.clone();
            let teams_title = teams_title.clone();
            let teams_intro = teams_intro.clone();
            Rc::new(move || {
                sidebar_i18n_refresh();
                profile_button
                    .set_tooltip_text(Some(crate::tr!("main-last-logins-tooltip").as_str()));
                profile_title.set_text(crate::tr!("main-last-logins-title").as_str());
                add_button.set_tooltip_text(Some(crate::tr!("main-add-tooltip").as_str()));
                trash_button.set_tooltip_text(Some(crate::tr!("main-trash-tooltip").as_str()));
                panic_button.set_tooltip_text(Some(crate::tr!("main-panic-tooltip").as_str()));
                panic_lbl.set_text(crate::tr!("main-panic-label").as_str());
                search_entry
                    .set_placeholder_text(Some(crate::tr!("main-search-placeholder").as_str()));
                status_total_chip
                    .set_tooltip_text(Some(crate::tr!("main-status-total-tooltip").as_str()));
                status_non_compliant_chip.set_tooltip_text(Some(
                    crate::tr!("main-status-noncompliant-tooltip").as_str(),
                ));
                sort_recent_button
                    .set_tooltip_text(Some(crate::tr!("main-sort-recent-tooltip").as_str()));
                sort_title_button
                    .set_tooltip_text(Some(crate::tr!("main-sort-title-tooltip").as_str()));
                sort_risk_button
                    .set_tooltip_text(Some(crate::tr!("main-sort-risk-tooltip").as_str()));

                filtered_status_page.set_title(crate::tr!("main-filtered-empty-title").as_str());
                filtered_status_page
                    .set_description(Some(crate::tr!("main-filtered-empty-description").as_str()));

                entries_stack
                    .page(&list_page)
                    .set_title(crate::tr!("main-stack-grid").as_str());
                entries_stack
                    .page(&empty_state)
                    .set_title(crate::tr!("main-stack-empty").as_str());

                main_stack
                    .page(&entries_stack)
                    .set_title(crate::tr!("main-stack-secrets").as_str());
                main_stack
                    .page(&profile_container)
                    .set_title(crate::tr!("main-profile-security").as_str());
                if is_admin {
                    main_stack
                        .page(&users_view_container)
                        .set_title(crate::tr!("main-user-nav").as_str());
                    main_stack
                        .page(&teams_view_container)
                        .set_title(crate::tr!("main-teams-nav").as_str());
                }
                main_stack
                    .page(&secret_editor_host)
                    .set_title(crate::tr!("main-stack-editor").as_str());

                users_back_button.set_label(crate::tr!("main-view-back").as_str());
                users_title.set_text(crate::tr!("main-users-view-title").as_str());
                users_intro.set_text(crate::tr!("main-users-view-intro").as_str());
                teams_back_button.set_label(crate::tr!("main-view-back").as_str());
                teams_title.set_text(crate::tr!("main-teams-view-title").as_str());
                teams_intro.set_text(crate::tr!("main-teams-view-intro").as_str());
            })
        };
        *on_main_i18n_refresh.borrow_mut() = Some(Rc::clone(&main_i18n_refresh));
        on_main_i18n_refresh_bridge();

        Self::update_sort_button_states(
            &center_panel.sort_recent_button,
            &center_panel.sort_title_button,
            &center_panel.sort_risk_button,
            filter_runtime.selected_sort.get(),
        );

        let flow_for_recent_sort = center_panel.secret_flow.clone();
        let filter_for_recent_sort = filter_runtime.clone();
        let recent_button = center_panel.sort_recent_button.clone();
        let title_button = center_panel.sort_title_button.clone();
        let risk_button = center_panel.sort_risk_button.clone();
        center_panel.sort_recent_button.connect_clicked(move |_| {
            filter_for_recent_sort
                .selected_sort
                .set(SecretSortMode::Recent);
            Self::update_sort_button_states(
                &recent_button,
                &title_button,
                &risk_button,
                SecretSortMode::Recent,
            );
            Self::apply_filters(&flow_for_recent_sort, &filter_for_recent_sort);
        });

        let flow_for_title_sort = center_panel.secret_flow.clone();
        let filter_for_title_sort = filter_runtime.clone();
        let recent_button = center_panel.sort_recent_button.clone();
        let title_button = center_panel.sort_title_button.clone();
        let risk_button = center_panel.sort_risk_button.clone();
        center_panel.sort_title_button.connect_clicked(move |_| {
            filter_for_title_sort
                .selected_sort
                .set(SecretSortMode::Title);
            Self::update_sort_button_states(
                &recent_button,
                &title_button,
                &risk_button,
                SecretSortMode::Title,
            );
            Self::apply_filters(&flow_for_title_sort, &filter_for_title_sort);
        });

        let flow_for_risk_sort = center_panel.secret_flow.clone();
        let filter_for_risk_sort = filter_runtime.clone();
        let recent_button = center_panel.sort_recent_button.clone();
        let title_button = center_panel.sort_title_button.clone();
        let risk_button = center_panel.sort_risk_button.clone();
        center_panel.sort_risk_button.connect_clicked(move |_| {
            filter_for_risk_sort.selected_sort.set(SecretSortMode::Risk);
            Self::update_sort_button_states(
                &recent_button,
                &title_button,
                &risk_button,
                SecretSortMode::Risk,
            );
            Self::apply_filters(&flow_for_risk_sort, &filter_for_risk_sort);
        });

        let flow_for_search = center_panel.secret_flow.clone();
        let filter_for_search = filter_runtime.clone();
        search_entry.connect_search_changed(move |entry| {
            *filter_for_search.search_text.borrow_mut() = entry.text().to_string();
            Self::apply_filters(&flow_for_search, &filter_for_search);
        });

        let active_vault_for_my_list = Rc::clone(&active_vault_id);
        let other_shared_list = sidebar_panel.shared_vaults_list.clone();
        let refresh_for_my_list = {
            let app = application.clone();
            let parent_window = window.clone();
            let runtime = runtime_handle.clone();
            let secret_service = Arc::clone(&secret_service);
            let vault_service = Arc::clone(&vault_service);
            let toast_overlay = toast_overlay.clone();
            let session_master = Rc::clone(&session_master_key);
            let active_vault_id = Rc::clone(&active_vault_id);
            let secret_flow = secret_flow_for_refresh.clone();
            let stack = stack_for_refresh.clone();
            let empty_title = empty_title_for_refresh.clone();
            let empty_copy = empty_copy_for_refresh.clone();
            let filter_runtime = filter_runtime.clone();
            let editor_launcher = Rc::clone(&editor_launcher);
            Rc::new(move || {
                let Some(master_key) = Self::snapshot_session_master_key(&session_master) else {
                    empty_title.set_text(crate::tr!("main-session-locked-title").as_str());
                    empty_copy.set_text(crate::tr!("main-session-locked-description").as_str());
                    stack.set_visible_child_name("empty");
                    return;
                };
                Self::refresh_secret_flow(
                    app.clone(),
                    parent_window.clone(),
                    runtime.clone(),
                    Arc::clone(&secret_service),
                    Arc::clone(&vault_service),
                    admin_user_id,
                    master_key,
                    secret_flow.clone(),
                    stack.clone(),
                    empty_title.clone(),
                    empty_copy.clone(),
                    active_vault_id.clone(),
                    toast_overlay.clone(),
                    filter_runtime.clone(),
                    editor_launcher.clone(),
                );
            })
        };
        let main_stack_for_my_list = center_panel.main_stack.clone();
        let sync_for_my_list = Rc::clone(&vault_selection_sync);
        let refresh_for_my_list_handler = Rc::clone(&refresh_for_my_list);
        let my_list_for_shared = sidebar_panel.my_vaults_list.clone();
        let active_vault_for_shared_list = Rc::clone(&active_vault_id);
        let sync_for_shared_list = Rc::clone(&vault_selection_sync);
        let refresh_for_shared_list_handler = Rc::clone(&refresh_for_my_list);
        let main_stack_for_shared_list = center_panel.main_stack.clone();
        sidebar_panel
            .my_vaults_list
            .connect_row_selected(move |_list, row_opt| {
                if sync_for_my_list.get() {
                    return;
                }
                if let Some(row) = row_opt {
                    if let Some(vault_id) = Self::vault_id_from_row(row) {
                        sync_for_my_list.set(true);
                        other_shared_list.unselect_all();
                        sync_for_my_list.set(false);
                        *active_vault_for_my_list.borrow_mut() = Some(vault_id);
                        main_stack_for_my_list.set_visible_child_name("entries_view");
                        refresh_for_my_list_handler();
                    }
                }
            });

        sidebar_panel
            .shared_vaults_list
            .connect_row_selected(move |_list, row_opt| {
                if sync_for_shared_list.get() {
                    return;
                }
                if let Some(row) = row_opt {
                    if let Some(vault_id) = Self::vault_id_from_row(row) {
                        sync_for_shared_list.set(true);
                        my_list_for_shared.unselect_all();
                        sync_for_shared_list.set(false);
                        *active_vault_for_shared_list.borrow_mut() = Some(vault_id);
                        main_stack_for_shared_list.set_visible_child_name("entries_view");
                        refresh_for_shared_list_handler();
                    }
                }
            });

        let flow_for_category = center_panel.secret_flow.clone();
        let filter_for_category = filter_runtime.clone();
        let main_stack_for_category = center_panel.main_stack.clone();
        sidebar_panel
            .category_list
            .connect_row_selected(move |_list, row_opt| {
                if let Some(row) = row_opt {
                    main_stack_for_category.set_visible_child_name("entries_view");
                    let category = match row.index() {
                        1 => SecretCategoryFilter::Password,
                        2 => SecretCategoryFilter::ApiToken,
                        3 => SecretCategoryFilter::SshKey,
                        4 => SecretCategoryFilter::SecureDocument,
                        _ => SecretCategoryFilter::All,
                    };
                    filter_for_category.selected_category.set(category);
                }
                Self::apply_filters(&flow_for_category, &filter_for_category);
            });

        let flow_for_audit = center_panel.secret_flow.clone();
        let filter_for_audit = filter_runtime.clone();
        let main_stack_for_audit = center_panel.main_stack.clone();
        sidebar_panel
            .audit_list
            .connect_row_selected(move |_list, row_opt| {
                if let Some(row) = row_opt {
                    main_stack_for_audit.set_visible_child_name("entries_view");
                    let audit = match row.index() {
                        1 => AuditFilter::Weak,
                        2 => AuditFilter::Duplicate,
                        _ => AuditFilter::All,
                    };
                    filter_for_audit.selected_audit.set(audit);
                }
                Self::apply_filters(&flow_for_audit, &filter_for_audit);
            });

        let key_controller = gtk4::EventControllerKey::new();
        let window_for_key = window.clone();
        let source_for_key = Rc::clone(&auto_lock_source);
        let armed_for_key = Rc::clone(&auto_lock_armed);
        let timeout_for_key = Rc::clone(&auto_lock_timeout_secs);
        let callback_for_key = Rc::clone(&on_auto_lock);
        let session_for_key = Rc::clone(&session_master_key);
        key_controller.connect_key_pressed(move |_controller, _key, _keycode, _state| {
            auto_lock::reset_auto_lock_timer(
                &window_for_key,
                &source_for_key,
                &armed_for_key,
                timeout_for_key.get(),
                &callback_for_key,
                &session_for_key,
            );
            glib::Propagation::Proceed
        });
        window.add_controller(key_controller);

        let motion_controller = gtk4::EventControllerMotion::new();
        let window_for_motion = window.clone();
        let source_for_motion = Rc::clone(&auto_lock_source);
        let armed_for_motion = Rc::clone(&auto_lock_armed);
        let timeout_for_motion = Rc::clone(&auto_lock_timeout_secs);
        let callback_for_motion = Rc::clone(&on_auto_lock);
        let session_for_motion = Rc::clone(&session_master_key);
        motion_controller.connect_motion(move |_controller, _x, _y| {
            auto_lock::reset_auto_lock_timer(
                &window_for_motion,
                &source_for_motion,
                &armed_for_motion,
                timeout_for_motion.get(),
                &callback_for_motion,
                &session_for_motion,
            );
        });
        window.add_controller(motion_controller);

        Self {
            window,
            secret_flow: secret_flow_for_struct,
            refresh_entries: Rc::clone(&refresh_list),
            auto_lock_timeout_secs,
            auto_lock_source,
            auto_lock_armed,
            session_master_key,
            on_auto_lock,
            on_logout,
            audit_service: Rc::new(audit_service),
        }
}

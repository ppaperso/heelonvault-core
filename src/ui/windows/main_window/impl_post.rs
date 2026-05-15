use super::*;

impl MainWindow {
    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    pub fn set_on_auto_lock(&self, callback: Rc<dyn Fn()>) {
        *self.on_auto_lock.borrow_mut() = Some(callback);
    }

    pub fn set_session_master_key(&self, key: Vec<u8>) {
        let mut current = self.session_master_key.borrow_mut();
        current.zeroize();
        *current = key;
    }

    pub fn refresh_entries(&self) {
        (self.refresh_entries)();
    }

    pub fn activate_auto_lock(&self) {
        auto_lock::activate_auto_lock(
            &self.window,
            &self.auto_lock_timeout_secs,
            &self.auto_lock_source,
            &self.auto_lock_armed,
            &self.on_auto_lock,
            &self.session_master_key,
        );
    }

    pub fn set_auto_lock_timeout(&self, mins: u64) {
        auto_lock::set_auto_lock_timeout(
            &self.window,
            &self.auto_lock_timeout_secs,
            &self.auto_lock_source,
            &self.auto_lock_armed,
            &self.on_auto_lock,
            &self.session_master_key,
            mins,
        );
    }

    pub fn deactivate_auto_lock(&self) {
        auto_lock::deactivate_auto_lock(&self.auto_lock_source, &self.auto_lock_armed);
    }

    pub fn clear_sensitive_session(&self) {
        self.deactivate_auto_lock();
        {
            let mut key = self.session_master_key.borrow_mut();
            key.zeroize();
            key.clear();
        }
        while let Some(child) = self.secret_flow.first_child() {
            self.secret_flow.remove(&child);
        }
    }

    pub fn set_on_logout(&self, callback: Rc<dyn Fn()>) {
        *self.on_logout.borrow_mut() = Some(callback);
    }

    pub fn trigger_logout(&self) {
        if let Some(callback) = self.on_logout.borrow().as_ref() {
            callback();
        }
    }

    pub(in crate::ui::windows::main_window) fn snapshot_session_master_key(
        session_master_key: &Rc<RefCell<Vec<u8>>>,
    ) -> Option<Vec<u8>> {
        let key = session_master_key.borrow();
        if key.is_empty() {
            None
        } else {
            Some(key.clone())
        }
    }

    pub(in crate::ui::windows::main_window) fn apply_filters(
        secret_flow: &gtk4::FlowBox,
        filter_runtime: &FilterRuntime,
    ) {
        search_filter::apply_filters(secret_flow, filter_runtime);
    }

    pub(in crate::ui::windows::main_window) fn update_sort_button_states(
        recent_button: &gtk4::Button,
        title_button: &gtk4::Button,
        risk_button: &gtk4::Button,
        selected_sort: SecretSortMode,
    ) {
        for button in [recent_button, title_button, risk_button] {
            button.remove_css_class("vault-secret-sort-button-active");
        }

        match selected_sort {
            SecretSortMode::Recent => {
                recent_button.add_css_class("vault-secret-sort-button-active")
            }
            SecretSortMode::Title => title_button.add_css_class("vault-secret-sort-button-active"),
            SecretSortMode::Risk => risk_button.add_css_class("vault-secret-sort-button-active"),
        }
    }

    pub(in crate::ui::windows::main_window) fn parse_search_terms(
        query: &str,
    ) -> Vec<(Option<String>, String)> {
        search_filter::parse_search_terms(query)
    }

    pub(in crate::ui::windows::main_window) fn matches_search_term(
        meta: &SecretFilterMeta,
        term: &(Option<String>, String),
    ) -> bool {
        search_filter::matches_search_term(meta, term)
    }

    pub(in crate::ui::windows::main_window) fn show_feedback_dialog(
        parent: &adw::ApplicationWindow,
        title: &str,
        body: &str,
    ) {
        let dialog = adw::MessageDialog::new(Some(parent), Some(title), Some(body));
        dialog.add_response("ok", crate::tr!("common-ok").as_str());
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        dialog.present();
    }

    pub(in crate::ui::windows::main_window) fn build_certification_menu_item(
        icon_name: &str,
        label: &str,
    ) -> gtk4::Button {
        certification::build_certification_menu_item(icon_name, label)
    }

    pub(in crate::ui::windows::main_window) fn show_certification_diagnostics_dialog(
        parent: &adw::ApplicationWindow,
        license_service: Arc<LicenseService>,
    ) {
        certification::show_certification_diagnostics_dialog(
            parent,
            license_service,
            Rc::new(|dialog_parent, title, body| {
                Self::show_feedback_dialog(dialog_parent, title, body);
            }),
        );
    }

    pub(in crate::ui::windows::main_window) fn set_inline_status(
        label: &gtk4::Label,
        message: &str,
        kind: &str,
    ) {
        label.remove_css_class("inline-status-loading");
        label.remove_css_class("inline-status-success");
        label.remove_css_class("inline-status-error");
        match kind {
            "loading" => label.add_css_class("inline-status-loading"),
            "success" => label.add_css_class("inline-status-success"),
            _ => label.add_css_class("inline-status-error"),
        }
        label.set_text(message);
        label.set_visible(true);

        if kind != "loading" {
            let label_for_hide = label.clone();
            glib::timeout_add_local_once(Duration::from_millis(3200), move || {
                label_for_hide.set_visible(false);
            });
        }
    }

    pub(in crate::ui::windows::main_window) fn set_twofa_badge_state(
        label: &gtk4::Label,
        enabled: bool,
    ) {
        label.remove_css_class("status-role-admin");
        label.remove_css_class("status-role-user");
        if enabled {
            let text = messages::twofa_badge_enabled();
            label.set_text(text.as_str());
            label.add_css_class("status-role-admin");
        } else {
            let text = messages::twofa_badge_disabled();
            label.set_text(text.as_str());
            label.add_css_class("status-role-user");
        }
    }

    pub(in crate::ui::windows::main_window) fn map_twofa_error(
        error: &crate::errors::AppError,
        fallback: &str,
    ) -> String {
        match error {
            crate::errors::AppError::Authorization(_) => {
                crate::tr!("twofa-error-invalid-clock")
            }
            crate::errors::AppError::Validation(message) => {
                if message.to_ascii_lowercase().contains("code") {
                    crate::tr!("twofa-error-invalid-clock")
                } else {
                    crate::tr!("twofa-error-invalid-setup")
                }
            }
            crate::errors::AppError::Storage(_)
            | crate::errors::AppError::Database(_)
            | crate::errors::AppError::Io(_) => {
                crate::tr!("twofa-error-storage")
            }
            crate::errors::AppError::Crypto(_) => {
                crate::tr!("twofa-error-crypto")
            }
            _ => fallback.to_string(),
        }
    }

    pub(in crate::ui::windows::main_window) fn format_login_timestamp_fr(raw: &str) -> String {
        const MONTHS: [&str; 12] = [
            "janvier",
            "fevrier",
            "mars",
            "avril",
            "mai",
            "juin",
            "juillet",
            "aout",
            "septembre",
            "octobre",
            "novembre",
            "decembre",
        ];

        let parsed_local = DateTime::parse_from_rfc3339(raw)
            .map(|value| value.with_timezone(&Local))
            .or_else(|_| {
                NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S").map(|naive| {
                    DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc).with_timezone(&Local)
                })
            })
            .or_else(|_| {
                NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").map(|naive| {
                    DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc).with_timezone(&Local)
                })
            });

        match parsed_local {
            Ok(value) => {
                let month_label = MONTHS
                    .get(value.month0() as usize)
                    .copied()
                    .unwrap_or("mois");
                format!(
                    "{} {} {} - {:02}h{:02}",
                    value.day(),
                    month_label,
                    value.year(),
                    value.hour(),
                    value.minute()
                )
            }
            Err(_) => raw.to_string(),
        }
    }

    pub(in crate::ui::windows::main_window) fn refresh_login_history_popover(
        runtime_handle: Handle,
        database_pool: SqlitePool,
        user_id: Uuid,
        list_box: gtk4::Box,
    ) {
        while let Some(child) = list_box.first_child() {
            list_box.remove(&child);
        }

        let loading_label = gtk4::Label::new(Some(crate::tr!("login-history-loading").as_str()));
        loading_label.set_halign(Align::Start);
        loading_label.add_css_class("profile-login-history-muted");
        list_box.append(&loading_label);

        let (sender, receiver) = tokio::sync::oneshot::channel();
        runtime_handle.spawn(async move {
            let result = list_recent_logins(&database_pool, user_id, 5).await;
            match result {
                Ok(entries) => {
                    let _ = sender.send(Ok(entries));
                }
                Err(primary_err) => {
                    warn!(
                        user_id = %user_id,
                        error = %primary_err,
                        "login_history service path failed, falling back to direct SQL query"
                    );
                    let fallback_rows = sqlx::query(
                        "SELECT login_at, ip_address, device_info
						 FROM login_history
						 WHERE user_id = ?1
						 ORDER BY login_at DESC
						 LIMIT 5",
                    )
                    .bind(user_id.to_string())
                    .fetch_all(&database_pool)
                    .await;

                    match fallback_rows {
                        Ok(rows) => {
                            let mut entries = Vec::with_capacity(rows.len());
                            for row in rows {
                                let login_at: String = row.try_get("login_at").unwrap_or_default();
                                let ip_address: Option<String> =
                                    row.try_get("ip_address").ok().flatten();
                                let device_info: Option<String> =
                                    row.try_get("device_info").ok().flatten();
                                entries.push(
                                    crate::services::login_history_service::LoginHistoryEntry {
                                        login_at,
                                        ip_address,
                                        device_info,
                                    },
                                );
                            }
                            let _ = sender.send(Ok(entries));
                        }
                        Err(fallback_err) => {
                            warn!(
                                user_id = %user_id,
                                error = %fallback_err,
                                "login_history fallback SQL query failed"
                            );
                            let _ = sender.send(Err(primary_err));
                        }
                    }
                }
            }
        });

        glib::MainContext::default().spawn_local(async move {
			while let Some(child) = list_box.first_child() {
				list_box.remove(&child);
			}

			match receiver.await {
				Ok(Ok(entries)) => {
					info!(user_id = %user_id, count = entries.len(), "login history loaded for popover");
					if entries.is_empty() {
						let row_label = gtk4::Label::new(Some(crate::tr!("login-history-empty").as_str()));
						row_label.set_halign(Align::Start);
						row_label.add_css_class("profile-login-history-muted");
						list_box.append(&row_label);
						return;
					}

					for entry in entries {
						let formatted_login = Self::format_login_timestamp_fr(entry.login_at.as_str());
						let mut line = formatted_login;
						if let Some(device) = entry
							.device_info
							.as_deref()
							.filter(|value| !value.trim().is_empty())
						{
							line.push_str("  •  ");
							line.push_str(device);
						} else if let Some(ip) = entry
							.ip_address
							.as_deref()
							.filter(|value| !value.trim().is_empty())
						{
							line.push_str("  •  ");
							line.push_str(ip);
						}

						let row_label = gtk4::Label::new(Some(line.as_str()));
						row_label.set_halign(Align::Start);
						row_label.set_xalign(0.0);
						row_label.add_css_class("profile-login-history-row");
						list_box.append(&row_label);
					}
				}
				_ => {
					let row_label = gtk4::Label::new(Some(crate::tr!("login-history-unavailable").as_str()));
					row_label.set_halign(Align::Start);
					row_label.add_css_class("profile-login-history-muted");
					list_box.append(&row_label);
				}
			}
		});
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::ui::windows::main_window) fn build_profile_view<
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
        profile_badge: gtk4::MenuButton,
        critical_ops_in_flight: Rc<Cell<u32>>,
        auto_lock_timeout_secs: Rc<Cell<u64>>,
        auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
        auto_lock_armed: Rc<Cell<bool>>,
        on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
        session_master_key: Rc<RefCell<Vec<u8>>>,
        show_passwords_in_edit_pref: Rc<Cell<bool>>,
        on_import_completed_refresh: Rc<dyn Fn()>,
        on_language_changed: Rc<dyn Fn()>,
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
        profile_view::build_profile_view(
            window,
            runtime_handle,
            user_service,
            totp_service,
            auth_policy_service,
            backup_service,
            backup_app_service,
            import_service,
            secret_service,
            vault_service,
            database_path,
            user_id,
            is_admin,
            profile_badge,
            critical_ops_in_flight,
            auto_lock_timeout_secs,
            auto_lock_source,
            auto_lock_armed,
            on_auto_lock,
            session_master_key,
            show_passwords_in_edit_pref,
            on_import_completed_refresh,
            on_language_changed,
        )
    }

    pub(in crate::ui::windows::main_window) fn build_sidebar_panel() -> SidebarWidgets {
        sidebar::build_sidebar_panel()
    }

    pub(in crate::ui::windows::main_window) fn build_vault_sidebar_row(
        title: &str,
        vault_id: Uuid,
        can_delete: bool,
        is_shared_with_others: bool,
        shared_role: Option<crate::models::VaultShareRole>,
        secret_count: usize,
        on_delete: Option<Rc<dyn Fn(Uuid, String)>>,
    ) -> gtk4::ListBoxRow {
        sidebar::build_vault_sidebar_row(
            title,
            vault_id,
            can_delete,
            is_shared_with_others,
            shared_role,
            secret_count,
            on_delete,
        )
    }

    pub(in crate::ui::windows::main_window) fn vault_id_from_row(
        row: &gtk4::ListBoxRow,
    ) -> Option<Uuid> {
        row.widget_name()
            .strip_prefix("vault-")
            .and_then(|raw| Uuid::parse_str(raw).ok())
    }

    pub(in crate::ui::windows::main_window) fn find_vault_row(
        list: &gtk4::ListBox,
        vault_id: Uuid,
    ) -> Option<gtk4::ListBoxRow> {
        let mut child_opt = list.first_child();
        while let Some(child) = child_opt {
            let next = child.next_sibling();
            if let Ok(row) = child.clone().downcast::<gtk4::ListBoxRow>() {
                if Self::vault_id_from_row(&row) == Some(vault_id) {
                    return Some(row);
                }
            }
            child_opt = next;
        }
        None
    }

    pub(in crate::ui::windows::main_window) fn build_center_panel() -> CenterPanelWidgets {
        center::build_center_panel()
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::ui::windows::main_window) fn refresh_secret_flow<TSecret, TVault>(
        application: adw::Application,
        parent_window: adw::ApplicationWindow,
        runtime_handle: Handle,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        admin_user_id: Uuid,
        admin_master_key: Vec<u8>,
        secret_flow: gtk4::FlowBox,
        stack: gtk4::Stack,
        empty_title: gtk4::Label,
        empty_copy: gtk4::Label,
        active_vault_id: Rc<RefCell<Option<Uuid>>>,
        toast_overlay: adw::ToastOverlay,
        filter_runtime: FilterRuntime,
        editor_launcher: Rc<RefCell<Option<Rc<dyn Fn(DialogMode)>>>>,
    ) where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        secret_flow::refresh_secret_flow(
            application,
            parent_window,
            runtime_handle,
            secret_service,
            vault_service,
            admin_user_id,
            admin_master_key,
            secret_flow,
            stack,
            empty_title,
            empty_copy,
            active_vault_id,
            toast_overlay,
            filter_runtime,
            editor_launcher,
        );
    }
}

use super::*;
use heelonvault_core::services::pin_cache_service::PinUnlockError;

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

    /// Store a freshly created `PinCache`, replacing any previous one.
    // Phase 5a: PinSetupDialog result not yet wired to MainWindow. Owner: ppaadmin | Due: Phase 5b
    /// Remove and zeroize the PIN cache (ZeroizeOnDrop ensures secure wipe).
    pub fn clear_pin_cache(&self) {
        let _ = self.pin_cache.borrow_mut().take();
        if let Some(cb) = self.on_pin_state_cb.borrow().as_ref() {
            cb(false);
        }
    }

    /// Returns `true` if a valid (non-exhausted, non-expired) PIN cache exists
    /// for the given user.
    pub fn has_pin_cache(&self, user_id: uuid::Uuid, hard_timeout: std::time::Duration) -> bool {
        if let Some(ref c) = *self.pin_cache.borrow() {
            c.user_id() == user_id && !c.is_exhausted() && !c.is_expired(hard_timeout)
        } else {
            false
        }
    }

    /// Try to unlock with the given PIN.
    ///
    /// Returns the decrypted master key bytes on success, or `PinUnlockError`
    /// otherwise.  If `Exhausted` is returned the cache has already been
    /// cleared.
    pub fn try_pin_unlock(&self, pin: &str) -> Result<Zeroizing<Vec<u8>>, PinUnlockError> {
        let mut guard = self.pin_cache.borrow_mut();
        match guard.as_mut() {
            None => Err(PinUnlockError::Exhausted),
            Some(cache) => match cache.try_unwrap(pin) {
                Ok(key) => Ok(key),
                Err(PinUnlockError::Exhausted) => {
                    drop(guard);
                    self.clear_pin_cache();
                    Err(PinUnlockError::Exhausted)
                }
                Err(e) => Err(e),
            },
        }
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
        crate::ui::sensitive_clipboard::clear_now();
        self.deactivate_auto_lock();
        self.clear_pin_cache();
        {
            let mut key = self.session_master_key.borrow_mut();
            key.zeroize();
            key.clear();
        }
        while let Some(child) = self.secret_flow.first_child() {
            self.secret_flow.remove(&child);
        }
    }

    /// Zeroes the session master key and disarms the auto-lock timer,
    /// WITHOUT wiping the PIN cache. Used when the user locks manually
    /// while a valid PIN cache exists so that re-entry uses PIN.
    pub fn lock_session_keep_pin(&self) {
        crate::ui::sensitive_clipboard::clear_now();
        self.deactivate_auto_lock();
        {
            let mut key = self.session_master_key.borrow_mut();
            key.zeroize();
            key.clear();
        }
    }

    pub fn set_on_logout(&self, callback: Rc<dyn Fn()>) {
        *self.on_logout.borrow_mut() = Some(callback);
    }

    pub fn set_on_pin_lock(&self, callback: Rc<dyn Fn()>) {
        *self.on_pin_lock.borrow_mut() = Some(callback);
    }

    pub fn trigger_pin_lock(&self) {
        // Auto-lock reaches the PIN dialog without going through lock_session_keep_pin.
        crate::ui::sensitive_clipboard::clear_now();
        if let Some(callback) = self.on_pin_lock.borrow().as_ref() {
            callback();
        }
    }

    pub fn session_user_id(&self) -> uuid::Uuid {
        self.session_user_id
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

    pub(in crate::ui::windows::main_window) fn show_feedback_dialog(
        parent: &adw::ApplicationWindow,
        title: &str,
        body: &str,
    ) {
        let dialog = adw::MessageDialog::new(Some(parent), Some(title), Some(body));
        dialog.add_response("ok", heelonvault_core::tr!("common-ok").as_str());
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        dialog.present();
    }

    // Phase 5a: not yet called from main.rs service error paths. Owner: ppaadmin | Due: Phase 5b
    // Phase 5a: not yet called from main.rs service error paths. Owner: ppaadmin | Due: Phase 5b
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

    pub fn refresh_login_history_popover(
        runtime_handle: Handle,
        database_pool: SqlitePool,
        user_id: Uuid,
        list_box: gtk4::Box,
    ) {
        while let Some(child) = list_box.first_child() {
            list_box.remove(&child);
        }

        let loading_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("login-history-loading").as_str(),
        ));
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
                                    heelonvault_core::services::login_history_service::LoginHistoryEntry {
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
						let row_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-history-empty").as_str()));
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
					let row_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-history-unavailable").as_str()));
					row_label.set_halign(Align::Start);
					row_label.add_css_class("profile-login-history-muted");
					list_box.append(&row_label);
				}
			}
		});
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
        error: &heelonvault_core::errors::AppError,
        fallback: &str,
    ) -> String {
        match error {
            heelonvault_core::errors::AppError::Authorization(_) => {
                heelonvault_core::tr!("twofa-error-invalid-clock")
            }
            heelonvault_core::errors::AppError::Validation(message) => {
                if message.to_ascii_lowercase().contains("code") {
                    heelonvault_core::tr!("twofa-error-invalid-clock")
                } else {
                    heelonvault_core::tr!("twofa-error-invalid-setup")
                }
            }
            heelonvault_core::errors::AppError::Storage(_)
            | heelonvault_core::errors::AppError::Database(_)
            | heelonvault_core::errors::AppError::Io(_) => {
                heelonvault_core::tr!("twofa-error-storage")
            }
            heelonvault_core::errors::AppError::Crypto(_) => {
                heelonvault_core::tr!("twofa-error-crypto")
            }
            _ => fallback.to_string(),
        }
    }
}

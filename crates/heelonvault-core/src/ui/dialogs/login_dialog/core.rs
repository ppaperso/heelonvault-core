use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, InputPurpose, Justification, Orientation};
use libadwaita as adw;
use secrecy::SecretBox;
use tokio::runtime::Handle;
use tracing::info;
use tracing::warn;

use super::{
    bootstrap_flow, feedback, lock_state, login_flow, restore_flow, window_state,
    AuthenticatedSession, BootstrapServicesContext, LoginAttemptOutcome, LoginDialog,
};
use crate::errors::{AccessDeniedReason, AppError};
use crate::services::auth_policy_service::AuthPolicyService;
use crate::services::auth_service::AuthService;
use crate::services::totp_service::TotpService;
use crate::services::user_service::UserService;
use crate::ui::widgets::password_strength_bar::PasswordStrengthBar;

impl LoginDialog {
    fn professional_customer_name(license_badge_text: &str) -> Option<String> {
        for prefix in [
            "Licence pro - ",
            "Pro: ",
            "Professional - ",
            "Professional: ",
        ] {
            if let Some(customer_name) = license_badge_text.strip_prefix(prefix) {
                let normalized = customer_name.trim();
                if !normalized.is_empty() {
                    return Some(normalized.to_ascii_uppercase());
                }
            }
        }

        None
    }

    pub fn new<TAuth, TPolicy, TUser, TTotp>(
        application: &adw::Application,
        parent: &adw::ApplicationWindow,
        runtime_handle: Handle,
        auth_service: Arc<TAuth>,
        auth_policy_service: Arc<TPolicy>,
        user_service: Arc<TUser>,
        totp_service: Arc<TTotp>,
        bootstrap_ctx: Option<BootstrapServicesContext>,
        license_badge_text: String,
        on_restore_requested: impl Fn(PathBuf, String, String) -> Result<(), AppError>
            + Send
            + Sync
            + 'static,
        on_restore_completed: impl Fn() + 'static,
        on_authenticated: impl Fn(AuthenticatedSession) + 'static,
        on_cancelled: impl Fn() + 'static,
    ) -> Self
    where
        TAuth: AuthService + Send + Sync + 'static,
        TPolicy: AuthPolicyService + Send + Sync + 'static,
        TUser: UserService + Send + Sync + 'static,
        TTotp: TotpService + Send + Sync + 'static,
    {
        include!("parts/new_body.inc")
    }

    pub fn present(&self) {
        self.window.present();
    }

    fn connect_feedback_reset<TWidget>(
        widget: &TWidget,
        error_label: &gtk4::Label,
        lock_active: Rc<Cell<bool>>,
    ) where
        TWidget: IsA<gtk4::Editable> + Clone + 'static,
    {
        let error_for_reset = error_label.clone();
        widget.connect_changed(move |_| {
            if lock_active.get() {
                return;
            }
            Self::clear_feedback(&error_for_reset);
        });
    }

    fn start_lock_countdown(
        button: &gtk4::Button,
        spinner: &gtk4::Spinner,
        error_label: &gtk4::Label,
        remaining_secs: i64,
        lock_active: Rc<Cell<bool>>,
        lock_timer: Rc<RefCell<Option<glib::SourceId>>>,
    ) {
        lock_state::start_lock_countdown(
            button,
            spinner,
            error_label,
            remaining_secs,
            lock_active,
            lock_timer,
            Self::set_pending_state,
            Self::show_feedback,
        );
    }

    fn clear_feedback(error_label: &gtk4::Label) {
        feedback::clear_feedback(error_label);
    }

    fn show_feedback(error_label: &gtk4::Label, message: &str) {
        feedback::show_feedback(error_label, message);
    }

    fn update_greeting(title_label: &gtk4::Label, username: &str) {
        feedback::update_greeting(title_label, username);
    }

    fn update_strength_feedback(password: &str, strength_label: &gtk4::Label) {
        feedback::update_strength_feedback(password, strength_label);
    }

    fn set_pending_state(button: &gtk4::Button, spinner: &gtk4::Spinner, pending: bool) {
        feedback::set_pending_state(button, spinner, pending);
    }

    fn present_restore_dialog(
        parent: &gtk4::Window,
        on_restore_requested: Arc<
            dyn Fn(PathBuf, String, String) -> Result<(), AppError> + Send + Sync,
        >,
        on_restore_completed: Rc<dyn Fn()>,
    ) {
        restore_flow::present_restore_dialog(parent, on_restore_requested, on_restore_completed);
    }
}

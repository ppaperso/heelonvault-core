use heelonvault_core::i18n::{I18nArg, tr, tr_args};

#[allow(dead_code)]
pub fn twofa_badge_disabled() -> String {
    tr("twofa-badge-disabled")
}

#[allow(dead_code)]
pub fn twofa_badge_enabled() -> String {
    tr("twofa-badge-enabled")
}

#[allow(dead_code)]
pub fn profile_totp_code_invalid_error() -> String {
    tr("profile-totp-code-invalid-error")
}

pub fn login_totp_code_missing_error() -> String {
    tr("login-totp-code-missing-error")
}

pub fn login_totp_code_invalid_error() -> String {
    tr("login-totp-code-invalid-error")
}

#[allow(dead_code)]
pub fn validate_totp_code_format(code: &str) -> Option<String> {
    heelonvault_core::i18n::validate_totp_code_format(code)
}

pub fn login_totp_error_message(code: &str) -> String {
    if code.trim().is_empty() {
        login_totp_code_missing_error()
    } else {
        login_totp_code_invalid_error()
    }
}

#[allow(dead_code)]
pub fn toast_secret_saved(name: &str) -> String {
    tr_args("toast-secret-saved", &[("name", I18nArg::Str(name))])
}

#[allow(dead_code)]
pub fn toast_secret_deleted(name: &str) -> String {
    tr_args("toast-secret-deleted", &[("name", I18nArg::Str(name))])
}

#[allow(dead_code)]
pub fn toast_password_copied(clear_after: std::time::Duration) -> String {
    tr_args(
        "toast-password-copied",
        &[("seconds", I18nArg::Num(clear_after.as_secs() as i64))],
    )
}

#[allow(dead_code)]
pub fn toast_login_copied(clear_after: std::time::Duration) -> String {
    tr_args(
        "toast-login-copied",
        &[("seconds", I18nArg::Num(clear_after.as_secs() as i64))],
    )
}

#[allow(dead_code)]
pub fn toast_url_opened() -> String {
    tr("toast-url-opened")
}

#[allow(dead_code)]
pub fn toast_url_opened_login_copied(clear_after: std::time::Duration) -> String {
    tr_args(
        "toast-url-opened-login-copied",
        &[("seconds", I18nArg::Num(clear_after.as_secs() as i64))],
    )
}

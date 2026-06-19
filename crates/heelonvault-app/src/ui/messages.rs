use heelonvault_core::i18n::{tr, tr_args, I18nArg};

pub fn twofa_badge_disabled() -> String {
    tr("twofa-badge-disabled")
}

pub fn twofa_badge_enabled() -> String {
    tr("twofa-badge-enabled")
}

pub fn profile_totp_code_invalid_error() -> String {
    tr("profile-totp-code-invalid-error")
}

pub fn login_totp_code_missing_error() -> String {
    tr("login-totp-code-missing-error")
}

pub fn login_totp_code_invalid_error() -> String {
    tr("login-totp-code-invalid-error")
}

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

pub fn toast_secret_saved(name: &str) -> String {
    tr_args("toast-secret-saved", &[("name", I18nArg::Str(name))])
}

pub fn toast_secret_deleted(name: &str) -> String {
    tr_args("toast-secret-deleted", &[("name", I18nArg::Str(name))])
}

pub fn toast_password_copied() -> String {
    tr("toast-password-copied")
}

pub fn toast_login_copied() -> String {
    tr("toast-login-copied")
}

pub fn toast_url_opened() -> String {
    tr("toast-url-opened")
}

pub fn toast_url_opened_login_copied() -> String {
    tr("toast-url-opened-login-copied")
}

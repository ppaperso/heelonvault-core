use crate::i18n::{tr, tr_args, I18nArg};

pub fn twofa_badge_disabled() -> String {
    tr("twofa-badge-disabled")
}

pub fn twofa_badge_enabled() -> String {
    tr("twofa-badge-enabled")
}

pub fn auth_error_title() -> String {
    tr("auth-error.title")
}

pub fn auth_error_description() -> String {
    tr("auth-error.description")
}

pub fn profile_totp_code_empty_error() -> String {
    tr("profile-totp-code-empty-error")
}

pub fn profile_totp_code_format_error() -> String {
    tr("profile-totp-code-format-error")
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
    if code.trim().is_empty() {
        return Some(profile_totp_code_empty_error());
    }

    if code.len() != 6 || !code.chars().all(|character| character.is_ascii_digit()) {
        return Some(profile_totp_code_format_error());
    }

    None
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

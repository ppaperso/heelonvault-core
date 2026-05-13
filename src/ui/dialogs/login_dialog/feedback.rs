use crate::i18n::I18nArg;
use gtk4::prelude::*;

pub(super) fn clear_feedback(error_label: &gtk4::Label) {
    error_label.set_text("");
    error_label.set_visible(false);
}

pub(super) fn show_feedback(error_label: &gtk4::Label, message: &str) {
    error_label.set_text(message);
    error_label.set_visible(true);
}

pub(super) fn update_greeting(title_label: &gtk4::Label, username: &str) {
    if username.is_empty() {
        title_label.set_text(crate::tr!("login-greeting-empty").as_str());
        return;
    }

    title_label.set_text(
        crate::i18n::tr_args(
            "login-greeting-hello",
            &[("username", I18nArg::Str(username))],
        )
        .as_str(),
    );
}

pub(super) fn update_strength_feedback(password: &str, strength_label: &gtk4::Label) {
    strength_label.remove_css_class("success");
    strength_label.remove_css_class("warning");
    strength_label.remove_css_class("error");

    if password.is_empty() {
        strength_label.set_text("");
        strength_label.set_visible(false);
        return;
    }

    let mut score = 0_u8;
    let length = password.chars().count();
    if length >= 12 {
        score += 2;
    } else if length >= 8 {
        score += 1;
    }

    let has_lower = password
        .chars()
        .any(|character| character.is_ascii_lowercase());
    let has_upper = password
        .chars()
        .any(|character| character.is_ascii_uppercase());
    let has_digit = password.chars().any(|character| character.is_ascii_digit());
    let has_special = password
        .chars()
        .any(|character| !character.is_ascii_alphanumeric());

    let complexity = [has_lower, has_upper, has_digit, has_special]
        .into_iter()
        .filter(|value| *value)
        .count();

    if complexity >= 3 {
        score += 2;
    } else if complexity >= 2 {
        score += 1;
    }

    let (label, css_class) = if score >= 4 {
        (crate::tr!("login-password-strength-very-strong"), "success")
    } else if score >= 3 {
        (crate::tr!("login-password-strength-strong"), "success")
    } else if score >= 2 {
        (crate::tr!("login-password-strength-medium"), "warning")
    } else {
        (crate::tr!("login-password-strength-weak"), "error")
    };

    strength_label.remove_css_class("dim-label");
    strength_label.add_css_class(css_class);
    strength_label.set_text(label.as_str());
    strength_label.set_visible(true);
}

pub(super) fn set_pending_state(button: &gtk4::Button, spinner: &gtk4::Spinner, pending: bool) {
    button.set_sensitive(!pending);
    spinner.set_visible(pending);
    spinner.set_spinning(pending);
}

pub(super) fn is_valid_totp(totp: &str) -> bool {
    totp.len() == 6 && totp.chars().all(|character| character.is_ascii_digit())
}

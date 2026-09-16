use heelonvault_core::i18n::{
    login_totp_error_message, profile_totp_code_invalid_error, set_language,
    validate_totp_code_format,
};

// These tests assert exact French message strings, but i18n::active_lang()
// falls back to the process LANG env var before defaulting to "fr" — CI
// runners don't set LANG=fr_FR, so each test pins the language explicitly
// instead of depending on the ambient environment.

#[test]
fn profile_empty_code_returns_expected_error_message() {
    set_language("fr");
    let message = validate_totp_code_format("");
    assert_eq!(
        message.as_deref(),
        Some("Code TOTP requis pour confirmer l'activation.")
    );
}

#[test]
fn profile_wrong_format_returns_expected_error_message() {
    set_language("fr");
    let non_digit_message = validate_totp_code_format("12a456");
    assert_eq!(
        non_digit_message.as_deref(),
        Some("Le code TOTP doit contenir exactement 6 chiffres.")
    );

    let short_message = validate_totp_code_format("12345");
    assert_eq!(
        short_message.as_deref(),
        Some("Le code TOTP doit contenir exactement 6 chiffres.")
    );
}

#[test]
fn profile_wrong_totp_code_message_is_exact() {
    set_language("fr");
    assert_eq!(
        profile_totp_code_invalid_error(),
        "Code TOTP invalide. Vérifiez votre application d'authentification."
    );
}

#[test]
fn login_missing_code_when_twofa_enabled_returns_expected_error_message() {
    set_language("fr");
    let message = login_totp_error_message("");
    assert_eq!(
        message.as_str(),
        "Code TOTP requis. Saisissez le code TOTP à 6 chiffres de votre application."
    );
}

#[test]
fn login_invalid_code_returns_expected_error_message() {
    set_language("fr");
    let message = login_totp_error_message("123456");
    assert_eq!(
        message.as_str(),
        "Code TOTP invalide. Vérifiez votre application d'authentification."
    );
}

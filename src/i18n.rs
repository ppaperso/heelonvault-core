#![allow(clippy::disallowed_methods)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};

use fluent_templates::fluent_bundle::FluentValue;
use fluent_templates::{static_loader, Loader};
use unic_langid::{langid, LanguageIdentifier};

static_loader! {
    static LOCALES = {
        locales: "locales",
        fallback_language: "en",
    };
}

static LANGUAGE_OVERRIDE: OnceLock<RwLock<String>> = OnceLock::new();

fn language_override() -> &'static RwLock<String> {
    LANGUAGE_OVERRIDE.get_or_init(|| RwLock::new(String::new()))
}

fn normalize_lang(input: &str) -> &str {
    // Accept common LANG values such as fr_FR.UTF-8 and keep the language code.
    let base = input.split('.').next().unwrap_or(input);
    base.split('_').next().unwrap_or(base)
}

fn active_lang() -> String {
    let lock = language_override();
    let from_override = match lock.read() {
        Ok(guard) => {
            if guard.trim().is_empty() {
                None
            } else {
                Some(guard.clone())
            }
        }
        Err(_) => None,
    };
    if let Some(value) = from_override {
        return value;
    }

    if let Ok(value) = std::env::var("HEELONVAULT_LANG") {
        return value;
    }

    if let Ok(value) = std::env::var("LANG") {
        return value;
    }

    "fr".to_string()
}

pub fn set_language(lang: &str) -> bool {
    let normalized = normalize_lang(lang).trim();
    if normalized.is_empty() {
        return false;
    }

    if LanguageIdentifier::from_str(normalized).is_err() {
        return false;
    }

    let lock = language_override();
    match lock.write() {
        Ok(mut guard) => {
            *guard = normalized.to_string();
            true
        }
        Err(_) => false,
    }
}

pub fn current_language() -> String {
    active_lang()
}

pub fn tr_with_lang(key: &str, lang: &str) -> String {
    let normalized = normalize_lang(lang);
    let lang_id = LanguageIdentifier::from_str(normalized).unwrap_or_else(|_| langid!("en"));
    LOCALES.lookup(&lang_id, key)
}

pub enum I18nArg<'a> {
    Str(&'a str),
    Num(i64),
}

pub fn tr_with_lang_args(key: &str, lang: &str, args: &[(&str, I18nArg<'_>)]) -> String {
    let normalized = normalize_lang(lang);
    let lang_id = LanguageIdentifier::from_str(normalized).unwrap_or_else(|_| langid!("en"));

    let mut fluent_args: HashMap<Cow<'static, str>, FluentValue<'_>> = HashMap::new();
    for (name, value) in args {
        let fluent_value = match value {
            I18nArg::Str(v) => FluentValue::from(*v),
            I18nArg::Num(v) => FluentValue::from(*v),
        };
        fluent_args.insert(Cow::Owned((*name).to_string()), fluent_value);
    }

    LOCALES.lookup_with_args(&lang_id, key, &fluent_args)
}

pub fn tr(key: &str) -> String {
    tr_with_lang(key, active_lang().as_str())
}

pub fn tr_args(key: &str, args: &[(&str, I18nArg<'_>)]) -> String {
    tr_with_lang_args(key, active_lang().as_str(), args)
}

#[macro_export]
macro_rules! tr {
    ($key:expr) => {{
        $crate::i18n::tr($key)
    }};
}

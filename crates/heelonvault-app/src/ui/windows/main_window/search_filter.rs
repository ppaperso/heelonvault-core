use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;

use super::{FilterRuntime, SecretFilterMeta};

pub(super) fn apply_filters(secret_flow: &gtk4::FlowBox, filter_runtime: &FilterRuntime) {
    let values = filter_runtime.meta_by_widget.borrow();

    let audit_all_count = values.len();
    let audit_weak_count = values.values().filter(|meta| meta.is_weak).count();
    let audit_duplicate_count = values.values().filter(|meta| meta.is_duplicate).count();
    let non_compliant_count = values
        .values()
        .filter(|meta| meta.is_weak || meta.is_duplicate)
        .count();

    update_audit_badge(&filter_runtime.audit_all_count_label, audit_all_count);
    update_audit_badge(&filter_runtime.audit_weak_count_label, audit_weak_count);
    update_audit_badge(
        &filter_runtime.audit_duplicate_count_label,
        audit_duplicate_count,
    );
    update_audit_badge(&filter_runtime.total_count_label, audit_all_count);
    update_audit_badge(
        &filter_runtime.non_compliant_count_label,
        non_compliant_count,
    );

    secret_flow.invalidate_sort();
    secret_flow.invalidate_filter();

    let mut visible_count = 0;
    let mut cursor = secret_flow.first_child();
    while let Some(child) = cursor {
        if let Some(flow_child) = child.downcast_ref::<gtk4::FlowBoxChild>() {
            if flow_child.is_child_visible() {
                visible_count += 1;
            }
        }
        cursor = child.next_sibling();
    }

    filter_runtime
        .filtered_status_page
        .set_visible(visible_count == 0);
}

pub(super) fn update_audit_badge(label: &gtk4::Label, value: usize) {
    let next_text = value.to_string();
    let current_text = label.text().to_string();
    if current_text == next_text {
        return;
    }

    label.set_text(&next_text);
    label.remove_css_class("audit-count-badge-pulse");
    label.add_css_class("audit-count-badge-pulse");

    let label_clone = label.clone();
    glib::timeout_add_local_once(Duration::from_millis(240), move || {
        label_clone.remove_css_class("audit-count-badge-pulse");
    });
}

pub(super) fn normalize_search_text(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => {
                'a'
            }
            'Ç' | 'ç' => 'c',
            'È' | 'É' | 'Ê' | 'Ë' | 'è' | 'é' | 'ê' | 'ë' => 'e',
            'Ì' | 'Í' | 'Î' | 'Ï' | 'ì' | 'í' | 'î' | 'ï' => 'i',
            'Ñ' | 'ñ' => 'n',
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => {
                'o'
            }
            'Ù' | 'Ú' | 'Û' | 'Ü' | 'ù' | 'ú' | 'û' | 'ü' => 'u',
            'Ý' | 'Ÿ' | 'ý' | 'ÿ' => 'y',
            'Æ' | 'æ' => 'a',
            'Œ' | 'œ' => 'o',
            _ => {
                if ch.is_whitespace() || ch == '-' || ch == '_' || ch == '.' || ch == '@' {
                    ' '
                } else {
                    continue;
                }
            }
        };
        normalized.push(mapped);
    }
    normalized
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

pub(super) fn within_one_edit(left: &str, right: &str) -> bool {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    let left_len = left_chars.len();
    let right_len = right_chars.len();

    if left_len.abs_diff(right_len) > 1 {
        return false;
    }

    let mut i = 0;
    let mut j = 0;
    let mut edits = 0_u8;

    while i < left_len && j < right_len {
        if left_chars[i] == right_chars[j] {
            i += 1;
            j += 1;
            continue;
        }

        edits += 1;
        if edits > 1 {
            return false;
        }

        if left_len > right_len {
            i += 1;
        } else if right_len > left_len {
            j += 1;
        } else {
            i += 1;
            j += 1;
        }
    }

    if i < left_len || j < right_len {
        edits += 1;
    }

    edits <= 1
}

pub(super) fn token_matches_haystack(token: &str, haystack: &str) -> bool {
    if token.is_empty() {
        return true;
    }

    if haystack.contains(token) {
        return true;
    }

    if token.chars().count() < 4 {
        return false;
    }

    haystack
        .split_whitespace()
        .any(|word| within_one_edit(token, word))
}

pub(super) fn parse_search_terms(query: &str) -> Vec<(Option<String>, String)> {
    // Pre-process: join tokens that end with ':' with the immediately following
    // token so that "field: value" (space after colon) is treated the same as
    // "field:value".
    let raw: Vec<&str> = query.split_whitespace().collect();
    let mut tokens: Vec<String> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let t = raw[i];
        if t.ends_with(':') && !t.starts_with(':') && i + 1 < raw.len() && !raw[i + 1].contains(':')
        {
            tokens.push(format!("{}{}", t, raw[i + 1]));
            i += 2;
        } else {
            tokens.push(t.to_string());
            i += 1;
        }
    }

    tokens
        .into_iter()
        .filter_map(|term| {
            let Some((raw_key, raw_value)) = term.split_once(':') else {
                let token = normalize_search_text(&term);
                if token.is_empty() {
                    return None;
                }
                return Some((None, token));
            };

            if raw_key.is_empty() || raw_value.is_empty() {
                let token = normalize_search_text(&term);
                if token.is_empty() {
                    return None;
                }
                return Some((None, token));
            }

            let value = normalize_search_text(raw_value);
            if value.is_empty() {
                return None;
            }
            let key_normalized = normalize_search_text(raw_key);

            let key = match key_normalized.as_str() {
                "title" | "titre" | "name" | "nom" => "title",
                "login" | "user" | "username" | "identifiant" => "login",
                "email" | "mail" => "email",
                "url" | "site" | "domaine" | "domain" => "url",
                "note" | "notes" => "notes",
                "category" | "categorie" | "cat" => "category",
                "tag" | "tags" => "tags",
                "type" | "kind" => "type",
                "vault" | "coffre" | "vault-name" => "vault",
                _ => return Some((None, normalize_search_text(&term))),
            };

            Some((Some(key.to_string()), value))
        })
        .filter(|(_, value)| !value.is_empty())
        .collect()
}

pub(super) fn matches_search_term(
    meta: &SecretFilterMeta,
    term: &(Option<String>, String),
) -> bool {
    let value = term.1.as_str();
    if value.is_empty() {
        return true;
    }

    match term.0.as_deref() {
        Some("title") => token_matches_haystack(value, meta.title_text.as_str()),
        Some("login") => token_matches_haystack(value, meta.login_text.as_str()),
        Some("email") => token_matches_haystack(value, meta.email_text.as_str()),
        Some("url") => token_matches_haystack(value, meta.url_text.as_str()),
        Some("notes") => token_matches_haystack(value, meta.notes_text.as_str()),
        Some("category") => token_matches_haystack(value, meta.category_text.as_str()),
        Some("tags") => token_matches_haystack(value, meta.tags_text.as_str()),
        Some("type") => token_matches_haystack(value, meta.type_text.as_str()),
        Some("vault") => token_matches_haystack(value, meta.vault_name_text.as_str()),
        _ => token_matches_haystack(value, meta.searchable_text.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::windows::main_window::SecretKind;

    fn make_meta(vault_name: &str) -> SecretFilterMeta {
        SecretFilterMeta {
            searchable_text: normalize_search_text(vault_name),
            title_text: String::new(),
            login_text: String::new(),
            email_text: String::new(),
            url_text: String::new(),
            notes_text: String::new(),
            category_text: String::new(),
            tags_text: String::new(),
            type_text: String::new(),
            vault_name_text: normalize_search_text(vault_name),
            kind: SecretKind::Password,
            original_rank: 0,
            is_weak: false,
            is_duplicate: false,
        }
    }

    #[test]
    fn vault_field_matches_exact() {
        let meta = make_meta("Perso");
        let terms = parse_search_terms("vault:perso");
        assert_eq!(terms.len(), 1);
        assert!(matches_search_term(&meta, &terms[0]));
    }

    #[test]
    fn coffre_alias_matches() {
        let meta = make_meta("Travail");
        let terms = parse_search_terms("coffre:travail");
        assert_eq!(terms.len(), 1);
        assert!(matches_search_term(&meta, &terms[0]));
    }

    #[test]
    fn vault_field_does_not_match_other_vault() {
        let meta = make_meta("Perso");
        let terms = parse_search_terms("vault:travail");
        assert!(!matches_search_term(&meta, &terms[0]));
    }

    #[test]
    fn vault_name_included_in_global_search() {
        let meta = make_meta("Banque");
        // Plain unqualified term should match via searchable_text (which includes vault name)
        let terms = parse_search_terms("banque");
        assert!(matches_search_term(&meta, &terms[0]));
    }
}

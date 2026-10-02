// Every message key the application passes literally to `tr!`, `tr` or `tr_args` must exist in
// the catalogs of heelonvault-core; an unknown key would be displayed raw to the user.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn app_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn catalog_keys(lang: &str) -> BTreeSet<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../heelonvault-core/locales")
        .join(lang)
        .join("main.ftl");
    let Ok(content) = std::fs::read_to_string(&path) else {
        panic!("cannot read {}", path.display());
    };
    content
        .lines()
        .filter(|line| line.starts_with(|c: char| c.is_ascii_lowercase()))
        .filter_map(|line| line.split_once(" ="))
        .map(|(key, _)| key.to_string())
        .collect()
}

fn source_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("cannot list {}", dir.display());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            source_files(&path, files);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "rs" || ext == "inc")
        {
            files.push(path);
        }
    }
}

/// Literal keys following `tr!(`, `tr(` or `tr_args(` (whitespace allowed before the quote).
fn literal_keys(source: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for call in ["tr!(", "tr(", "tr_args("] {
        let mut rest = source;
        while let Some(index) = rest.find(call) {
            let preceded_by_ident = rest[..index]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            rest = &rest[index + call.len()..];
            if preceded_by_ident {
                continue;
            }
            let Some(after_quote) = rest.trim_start().strip_prefix('"') else {
                continue;
            };
            if let Some(end) = after_quote.find('"') {
                keys.push(after_quote[..end].to_string());
            }
        }
    }
    keys
}

#[test]
fn every_literal_message_key_exists_in_both_catalogs() {
    let fr = catalog_keys("fr");
    let en = catalog_keys("en");
    let mut files = Vec::new();
    source_files(&app_src(), &mut files);

    let mut missing = Vec::new();
    for file in files {
        let Ok(source) = std::fs::read_to_string(&file) else {
            panic!("cannot read {}", file.display());
        };
        for key in literal_keys(&source) {
            if !fr.contains(&key) || !en.contains(&key) {
                missing.push(format!("{}: {key}", file.display()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "unknown message keys:\n{}",
        missing.join("\n")
    );
}

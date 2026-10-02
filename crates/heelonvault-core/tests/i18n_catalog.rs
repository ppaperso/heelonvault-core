// The French and English catalogs must define exactly the same message keys: a key missing
// from one language would silently fall back to the other at runtime.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn catalog_keys(lang: &str) -> BTreeSet<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("locales")
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

#[test]
fn french_and_english_catalogs_define_the_same_keys() {
    let fr = catalog_keys("fr");
    let en = catalog_keys("en");
    let only_fr: Vec<_> = fr.difference(&en).collect();
    let only_en: Vec<_> = en.difference(&fr).collect();
    assert!(
        only_fr.is_empty() && only_en.is_empty(),
        "keys only in fr: {only_fr:?}; keys only in en: {only_en:?}"
    );
}

#[test]
fn catalogs_have_no_duplicate_keys() {
    for lang in ["fr", "en"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("locales")
            .join(lang)
            .join("main.ftl");
        let Ok(content) = std::fs::read_to_string(&path) else {
            panic!("cannot read {}", path.display());
        };
        let mut seen = BTreeSet::new();
        for line in content.lines() {
            if let Some((key, _)) = line.split_once(" =")
                && line.starts_with(|c: char| c.is_ascii_lowercase())
            {
                assert!(seen.insert(key.to_string()), "{lang}: duplicate key {key}");
            }
        }
    }
}

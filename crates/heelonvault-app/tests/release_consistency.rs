// The product, the heelonvault-core crate published on crates.io and heelonvault-premium share one
// version number (see docs/internal/RELEASING.md). These tests fail as soon as a bump forgets one of them.

use std::path::{Path, PathBuf};

fn app_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    app_dir().join("../..")
}

fn read(path: &Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        panic!("cannot read {}", path.display());
    };
    content
}

/// Value of the first `key = "value"` line of `section` in a Cargo manifest.
fn manifest_value(manifest: &str, section: &str, key: &str) -> Option<String> {
    let header = format!("[{section}]");
    let mut in_section = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_section = line == header;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let value = value.trim();
        // Inline table form: `dep = { version = "2.0", features = [...] }`.
        let value = match value.strip_prefix('{') {
            Some(table) => table
                .split(',')
                .filter_map(|field| field.split_once('='))
                .find(|(field, _)| field.trim() == "version")
                .map_or("", |(_, version)| version),
            None => value,
        };
        return Some(value.trim().trim_matches('"').to_string());
    }
    None
}

fn package_version(manifest_path: &Path) -> String {
    let Some(version) = manifest_value(&read(manifest_path), "package", "version") else {
        panic!("no [package] version in {}", manifest_path.display());
    };
    version
}

fn numbers(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+'])
        .take(3)
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// Whether `version` satisfies the Cargo caret requirement `requirement` ("2.0", "2.0.0", "^2.0").
fn caret_matches(requirement: &str, version: &str) -> bool {
    let wanted = numbers(requirement.trim_start_matches('^'));
    let actual = numbers(version);
    let Some(&major) = wanted.first() else {
        return false;
    };
    actual.first() == Some(&major) && actual >= wanted
}

#[test]
fn caret_requirements_follow_cargo_rules() {
    assert!(caret_matches("2.0", "2.0.0"));
    assert!(caret_matches("2.0", "2.3.1"));
    assert!(caret_matches("^2.1.0", "2.1.0"));
    assert!(!caret_matches("2.1", "2.0.9"));
    assert!(!caret_matches("1.2", "2.0.0"));
    assert!(!caret_matches("2.0", "1.9.0"));
}

#[test]
fn manifest_values_are_read_from_plain_and_table_forms() {
    let manifest = "[package]\nversion = \"2.0.0\"\n\n[dependencies]\nplain = \"2.0\"\n\
                    table = { version = \"2.1\", features = [\"x\"] }\n";
    assert_eq!(
        manifest_value(manifest, "package", "version").as_deref(),
        Some("2.0.0")
    );
    assert_eq!(
        manifest_value(manifest, "dependencies", "plain").as_deref(),
        Some("2.0")
    );
    assert_eq!(
        manifest_value(manifest, "dependencies", "table").as_deref(),
        Some("2.1")
    );
    assert_eq!(manifest_value(manifest, "dependencies", "version"), None);
}

#[test]
fn app_and_core_share_the_product_version() {
    let core = package_version(&workspace_root().join("crates/heelonvault-core/Cargo.toml"));
    assert_eq!(env!("CARGO_PKG_VERSION"), core);
}

#[test]
fn premium_shares_the_version_and_depends_on_this_core() {
    let core = package_version(&workspace_root().join("crates/heelonvault-core/Cargo.toml"));
    let premium_manifest_path = workspace_root().join("../heelonvault-premium/Cargo.toml");
    let premium_manifest = read(&premium_manifest_path);

    assert_eq!(
        manifest_value(&premium_manifest, "package", "version").as_deref(),
        Some(core.as_str()),
        "heelonvault-premium must carry the product version"
    );
    let Some(requirement) = manifest_value(&premium_manifest, "dependencies", "heelonvault-core")
    else {
        panic!("heelonvault-premium does not depend on heelonvault-core");
    };
    assert!(
        caret_matches(&requirement, &core),
        "heelonvault-premium requires heelonvault-core {requirement}, which excludes {core}"
    );
}

#[test]
fn the_changelogs_document_the_current_version() {
    let core = package_version(&workspace_root().join("crates/heelonvault-core/Cargo.toml"));
    let heading = format!("## [{core}]");
    for changelog in [
        "docs/CHANGELOG.md",
        "docs/CHANGELOG.en.md",
        "../heelonvault-premium/CHANGELOG.md",
        "../heelonvault-premium/CHANGELOG.en.md",
    ] {
        assert!(
            read(&workspace_root().join(changelog)).contains(&heading),
            "{changelog} has no `{heading}` section"
        );
    }
}

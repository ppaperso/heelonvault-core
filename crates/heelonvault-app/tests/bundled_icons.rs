// Every bundled `hv-*-symbolic` icon the application names must be compiled into the
// GResource bundle, otherwise GTK silently renders an empty image. Conversely, each SVG of
// assets/icons/symbolic must be registered, so an icon is never added without being shipped.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

/// `hv-…-symbolic` names quoted in the sources.
fn icons_used() -> BTreeSet<String> {
    let mut files = Vec::new();
    source_files(&crate_dir().join("src"), &mut files);
    let mut used = BTreeSet::new();
    for file in files {
        let Ok(content) = std::fs::read_to_string(&file) else {
            panic!("cannot read {}", file.display());
        };
        for chunk in content.split('"').skip(1).step_by(2) {
            if chunk.starts_with("hv-")
                && chunk.ends_with("-symbolic")
                && chunk
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                used.insert(chunk.to_string());
            }
        }
    }
    used
}

/// Icon names registered under `scalable/actions/` in gresources.xml.
fn icons_registered() -> BTreeSet<String> {
    let path = crate_dir().join("assets/gresources.xml");
    let Ok(content) = std::fs::read_to_string(&path) else {
        panic!("cannot read {}", path.display());
    };
    content
        .split("alias=\"scalable/actions/")
        .skip(1)
        .filter_map(|rest| rest.split_once(".svg\""))
        .map(|(name, _)| name.to_string())
        .collect()
}

fn icons_on_disk() -> BTreeSet<String> {
    let dir = crate_dir().join("assets/icons/symbolic");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        panic!("cannot list {}", dir.display());
    };
    entries
        .flatten()
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".svg"))
                .map(str::to_string)
        })
        .collect()
}

#[test]
fn every_bundled_icon_used_by_the_code_is_registered() {
    let registered = icons_registered();
    let missing: Vec<_> = icons_used().difference(&registered).cloned().collect();
    assert!(
        missing.is_empty(),
        "icons used but not in gresources.xml: {missing:?}"
    );
}

#[test]
fn every_icon_file_is_registered_and_vice_versa() {
    assert_eq!(icons_on_disk(), icons_registered());
}

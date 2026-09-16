fn main() {
    // Les assets GTK GLib sont dans assets/ dans le dossier de la crate.
    // Ce build script s'exécute depuis crates/heelonvault-app/.
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(v) => v,
        Err(_) => panic!("CARGO_MANIFEST_DIR is always set by Cargo during build"),
    };
    let assets_dir = format!("{manifest_dir}/assets");
    let gresources_xml = format!("{assets_dir}/gresources.xml");

    glib_build_tools::compile_resources(
        &[assets_dir.as_str()],
        &gresources_xml,
        "heelonvault.gresource",
    );

    // Injection de l'édition compilée (community ou professional).
    if std::env::var_os("CARGO_FEATURE_PREMIUM").is_some() {
        println!("cargo:rustc-env=HEELONVAULT_EDITION=professional");
    } else {
        println!("cargo:rustc-env=HEELONVAULT_EDITION=community");
    }

    // Icône de heelonvault.exe (Explorer, barre des tâches, raccourci Start
    // Menu) — embarquée en ressource PE.
    embed_windows_icon(&manifest_dir);
}

// #[cfg(target_os = "windows")], not a runtime check: embed-resource is only
// a build-dependency under target."cfg(windows)" in Cargo.toml (see there),
// so referencing the crate unconditionally would fail to compile elsewhere.
#[cfg(target_os = "windows")]
fn embed_windows_icon(manifest_dir: &str) {
    println!("cargo:rerun-if-changed=windows/heelonvault.rc");
    println!("cargo:rerun-if-changed=assets/icons/Heelonys.ico");
    embed_resource::compile(
        format!("{manifest_dir}/windows/heelonvault.rc"),
        embed_resource::NONE,
    )
    .manifest_optional()
    .unwrap_or_else(|error| panic!("failed to embed Windows icon resource: {error}"));
}

#[cfg(not(target_os = "windows"))]
fn embed_windows_icon(_manifest_dir: &str) {}

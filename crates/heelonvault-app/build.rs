fn main() {
    // Les assets GTK GLib sont dans assets/ à la racine du workspace.
    // Ce build script s'exécute depuis crates/heelonvault-app/ → remonter de 2 niveaux.
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(v) => v,
        Err(_) => panic!("CARGO_MANIFEST_DIR is always set by Cargo during build"),
    };
    let assets_dir = format!("{manifest_dir}/../../assets");
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
}

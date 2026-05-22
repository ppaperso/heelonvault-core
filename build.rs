fn main() {
    glib_build_tools::compile_resources(
        &["assets"],
        "assets/gresources.xml",
        "heelonvault.gresource",
    );

    // Inject build-time edition identifier consumed via env!("HEELONVAULT_EDITION").
    if std::env::var_os("CARGO_FEATURE_PREMIUM").is_some() {
        println!("cargo:rustc-env=HEELONVAULT_EDITION=professional");
    } else {
        println!("cargo:rustc-env=HEELONVAULT_EDITION=community");
    }
}

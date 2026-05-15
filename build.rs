fn main() {
    glib_build_tools::compile_resources(
        &["assets"],
        "assets/gresources.xml",
        "heelonvault.gresource",
    );
}

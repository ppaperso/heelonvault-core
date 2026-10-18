//! Application look: stylesheets and bundled icons. Shared by the binary and the GTK smoke
//! test so both render exactly what users see.

use gtk4::gdk;

const RESOURCE_ROOT: &str = "/com/heelonvault/rust";

/// Registers the bundled `hv-*-symbolic` icons and the application stylesheets on `display`.
pub fn install(display: &gdk::Display) {
    // Bundled icons live under RESOURCE_ROOT/scalable/actions (see gresources.xml).
    gtk4::IconTheme::for_display(display).add_resource_path(RESOURCE_ROOT);

    add_stylesheet(display, "style.css");
    // libadwaita >= 1.6 reads its accent from CSS variables, which older GTK cannot parse:
    // the brand accent for those versions lives in its own stylesheet.
    if libadwaita::major_version() > 1 || libadwaita::minor_version() >= 6 {
        add_stylesheet(display, "style/accent-modern.css");
    }
}

fn add_stylesheet(display: &gdk::Display, path: &str) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_resource(&format!("{RESOURCE_ROOT}/{path}"));
    gtk4::style_context_add_provider_for_display(
        display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

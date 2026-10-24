//! Documentation capture uses production widgets with isolated test fixtures.
use gtk4::prelude::*;

pub(crate) fn language() -> Option<String> {
    std::env::var("HEELONVAULT_DOC_LANGUAGE").ok()
}

pub(crate) fn capture(window: &impl IsA<gtk4::Window>, name: &str) -> anyhow::Result<()> {
    let Some(language) = language() else {
        return Ok(());
    };
    if heelonvault_core::i18n::current_language() != language {
        return Ok(());
    }
    let root = std::env::var_os("HEELONVAULT_DOC_OUTPUT")
        .ok_or_else(|| anyhow::anyhow!("missing documentation output"))?;
    let directory = std::path::PathBuf::from(root).join(language);
    std::fs::create_dir_all(&directory)?;
    let window = window.as_ref();
    let context = gtk4::glib::MainContext::default();
    let until = std::time::Instant::now() + std::time::Duration::from_millis(400);
    while std::time::Instant::now() < until {
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    anyhow::ensure!(
        window.width() >= window.default_width() && window.height() >= window.default_height(),
        "display too small for {name}: {}x{}, requested {}x{}",
        window.width(),
        window.height(),
        window.default_width(),
        window.default_height()
    );
    let paintable = gtk4::WidgetPaintable::new(Some(window));
    let snapshot = gtk4::Snapshot::new();
    paintable.snapshot(
        &snapshot,
        f64::from(window.width()),
        f64::from(window.height()),
    );
    let node = snapshot
        .to_node()
        .ok_or_else(|| anyhow::anyhow!("empty documentation snapshot"))?;
    let surface = window
        .surface()
        .ok_or_else(|| anyhow::anyhow!("missing documentation surface"))?;
    let renderer = gtk4::gsk::Renderer::for_surface(&surface)
        .ok_or_else(|| anyhow::anyhow!("missing renderer"))?;
    renderer
        .render_texture(&node, None)
        .save_to_png(directory.join(format!("{name}.png")))?;
    renderer.unrealize();
    Ok(())
}

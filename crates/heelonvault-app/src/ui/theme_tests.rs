//! Run on the shared GTK thread, with the production stylesheet and bundled icons.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gtk4::{Orientation, StateFlags, glib, prelude::*};
use libadwaita as adw;

use super::widgets::secret_card::{SecretCard, SecretRowData, SecretViewMode};

fn settle() {
    let context = glib::MainContext::default();
    let until = Instant::now() + Duration::from_millis(200);
    while Instant::now() < until {
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn geometry(widget: &impl IsA<gtk4::Widget>) -> (i32, i32, (i32, i32, i32, i32)) {
    let widget = widget.as_ref();
    (
        widget.width(),
        widget.height(),
        widget.measure(Orientation::Horizontal, -1),
    )
}

fn assert_contrast(
    widget: &impl IsA<gtk4::Widget>,
    background: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fn luminance(color: gtk4::gdk::RGBA) -> f64 {
        let linear = |value: f32| {
            let value = f64::from(value);
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.red())
            + 0.7152 * linear(color.green())
            + 0.0722 * linear(color.blue())
    }
    let foreground = luminance(widget.as_ref().style_context().color());
    let background = luminance(gtk4::gdk::RGBA::parse(background)?);
    let contrast = (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05);
    assert!(
        contrast >= 4.5,
        "{}: contrast {contrast:.2} is below 4.5:1",
        widget.as_ref().css_name()
    );
    Ok(())
}

fn assert_text_contrast(
    widget: &impl IsA<gtk4::Widget>,
    background: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let widget = widget.as_ref();
    if widget.is::<gtk4::Label>() || widget.is::<gtk4::Image>() {
        assert_contrast(widget, background)?;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        assert_text_contrast(&current, background)?;
        child = current.next_sibling();
    }
    Ok(())
}

fn snapshot(window: &adw::Window, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    snapshot_widget(window, window, name)
}

fn snapshot_widget(
    window: &adw::Window,
    widget: &impl IsA<gtk4::Widget>,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let widget = widget.as_ref();
    if let Some(dir) = std::env::var_os("HEELONVAULT_UI_SNAPSHOT_DIR") {
        settle();
        let paintable = gtk4::WidgetPaintable::new(Some(widget));
        let snapshot = gtk4::Snapshot::new();
        paintable.snapshot(
            &snapshot,
            f64::from(widget.width()),
            f64::from(widget.height()),
        );
        let node = snapshot.to_node().ok_or("empty interaction snapshot")?;
        let surface = window.surface().ok_or("missing interaction surface")?;
        let renderer =
            gtk4::gsk::Renderer::for_surface(&surface).ok_or("missing interaction renderer")?;
        renderer
            .render_texture(&node, None)
            .save_to_png(PathBuf::from(dir).join(format!("interactions-{name}.png")))?;
        renderer.unrealize();
    }
    Ok(())
}

pub(crate) fn native_theme_scenario(
    profile: gtk4::MenuButton,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = gtk4::Box::new(Orientation::Vertical, 14);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    let actions = gtk4::Box::new(Orientation::Horizontal, 10);
    let primary = gtk4::Button::with_label("Enregistrer");
    primary.add_css_class("suggested-action");
    let secondary = gtk4::Button::with_label("Annuler");
    let destructive = gtk4::Button::with_label("Supprimer");
    destructive.add_css_class("destructive-action");
    let utility = gtk4::Button::from_icon_name("hv-copy-symbolic");
    utility.add_css_class("flat");
    for button in [&primary, &secondary, &destructive, &utility] {
        actions.append(button);
    }
    content.append(&actions);

    let multivault = gtk4::ToggleButton::with_label("MultiCoffre");
    multivault.add_css_class("flat");
    multivault.add_css_class("pill");
    multivault.set_halign(gtk4::Align::Start);
    content.append(&multivault);

    let selectors = gtk4::Box::new(Orientation::Horizontal, 4);
    selectors.add_css_class("vault-secret-sort-switch");
    let grid = gtk4::Button::from_icon_name("hv-grid-symbolic");
    let list = gtk4::Button::from_icon_name("hv-list-symbolic");
    for button in [&grid, &list] {
        button.add_css_class("flat");
        button.add_css_class("vault-secret-sort-button");
        selectors.append(button);
    }
    grid.add_css_class("vault-secret-sort-button-active");
    content.append(&selectors);

    let data = SecretRowData {
        secret_id: uuid::Uuid::new_v4(),
        icon_name: "hv-key-symbolic".into(),
        type_label: "Mot de passe".into(),
        title: "Messagerie professionnelle".into(),
        created_at: String::new(),
        login: "camille@example.test".into(),
        url: "https://example.test".into(),
        has_secret: true,
        is_password: true,
        color_class: "secret-type-password".into(),
        is_weak: false,
        is_health_access: false,
        usage_count: 3,
        is_duplicate: false,
        is_incomplete: false,
        is_shared_vault: false,
        can_edit: true,
        can_delete: true,
        vault_name: String::new(),
    };
    let card = SecretCard::new(data.clone(), SecretViewMode::Grid);
    let flow = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .build();
    let child = gtk4::FlowBoxChild::new();
    child.set_child(Some(&card.get_widget()));
    flow.insert(&child, -1);
    content.append(&flow);

    let popover = gtk4::Popover::new();
    let menu = gtk4::Box::new(Orientation::Vertical, 4);
    let menu_button = gtk4::MenuButton::builder()
        .label("Actions")
        .popover(&popover)
        .build();
    let row = gtk4::Box::new(Orientation::Horizontal, 10);
    row.append(&gtk4::Image::from_icon_name("hv-copy-symbolic"));
    row.append(&gtk4::Label::new(Some("Copier l’identifiant")));
    let copy = gtk4::Button::new();
    copy.set_child(Some(&row));
    copy.add_css_class("app-menu-item");
    menu.append(&copy);
    menu.append(&gtk4::Separator::new(Orientation::Horizontal));
    let remove = gtk4::Button::with_label("Supprimer");
    remove.add_css_class("app-menu-item");
    remove.add_css_class("app-menu-item-destructive");
    menu.append(&remove);
    popover.set_child(Some(&menu));
    content.append(&menu_button);
    let dropdown = gtk4::DropDown::from_strings(&["Personnel", "Professionnel", "Partagé"]);
    content.append(&dropdown);

    let header = adw::HeaderBar::new();
    header.pack_start(&profile);
    let help = gtk4::Button::from_icon_name("hv-info-symbolic");
    help.add_css_class("flat");
    header.pack_end(&help);
    let editor = gtk4::Box::new(Orientation::Vertical, 8);
    editor.add_css_class("add-edit-dialog");
    let hero = gtk4::Box::new(Orientation::Horizontal, 8);
    hero.add_css_class("login-hero");
    let back = gtk4::Button::with_label("Retour");
    back.add_css_class("flat");
    back.add_css_class("add-edit-back-btn");
    hero.append(&back);
    editor.append(&hero);
    let delete = gtk4::Button::new();
    delete.add_css_class("destructive-action");
    delete.add_css_class("add-edit-delete-btn");
    let delete_content = gtk4::Box::new(Orientation::Horizontal, 8);
    delete_content.append(&gtk4::Image::from_icon_name("hv-trash-symbolic"));
    delete_content.append(&gtk4::Label::new(Some("Supprimer")));
    delete.set_child(Some(&delete_content));
    delete.set_halign(gtk4::Align::Start);
    editor.append(&delete);
    content.append(&editor);
    let root = gtk4::Box::new(Orientation::Vertical, 0);
    root.append(&header);
    root.append(&content);
    let window = adw::Window::builder()
        .default_width(760)
        .default_height(560)
        .content(&root)
        .build();
    window.present();
    settle();
    let profile_button = profile
        .first_child()
        .and_downcast::<gtk4::ToggleButton>()
        .ok_or("missing profile toggle button")?;
    for flag in [
        StateFlags::NORMAL,
        StateFlags::PRELIGHT,
        StateFlags::ACTIVE,
        StateFlags::CHECKED,
        StateFlags::FOCUSED | StateFlags::FOCUS_VISIBLE,
    ] {
        for widget in [
            profile_button.upcast_ref::<gtk4::Widget>(),
            back.upcast_ref(),
            delete.upcast_ref(),
        ] {
            widget.set_state_flags(flag, false);
        }
        settle();
        // Least favorable background in each button's enabled state range.
        assert_text_contrast(&profile_button, "#D6F1E9")?;
        assert_text_contrast(&back, "#2C7573")?;
        assert_text_contrast(&delete, "#C01C28")?;
        for widget in [
            profile_button.upcast_ref::<gtk4::Widget>(),
            back.upcast_ref(),
            delete.upcast_ref(),
        ] {
            widget.unset_state_flags(flag);
        }
    }
    snapshot(&window, "contrast-controls")?;
    snapshot(&window, "rest")?;
    for button in [
        &primary,
        &secondary,
        &destructive,
        &utility,
        &grid,
        &list,
        &help,
    ] {
        let rest = geometry(button);
        for flag in [
            StateFlags::PRELIGHT,
            StateFlags::ACTIVE,
            StateFlags::FOCUSED | StateFlags::FOCUS_VISIBLE,
        ] {
            button.set_state_flags(flag, false);
            settle();
            assert_eq!(
                geometry(button),
                rest,
                "button geometry changed for {flag:?}"
            );
            button.unset_state_flags(flag);
        }
        button.set_sensitive(false);
        button.set_state_flags(StateFlags::PRELIGHT, false);
        settle();
        assert_eq!(geometry(button), rest, "disabled button geometry changed");
        assert!(!button.is_sensitive());
        button.unset_state_flags(StateFlags::PRELIGHT);
        button.set_sensitive(true);
    }
    let card_widget = card.get_widget();
    let title = card_widget.first_child().ok_or("missing card header")?;
    let bounds = title
        .compute_bounds(&flow)
        .ok_or("missing card header bounds")?;
    let rest = geometry(&card_widget);
    card_widget.set_state_flags(StateFlags::PRELIGHT, false);
    settle();
    assert_eq!(geometry(&card_widget), rest);
    assert_eq!(
        title.compute_bounds(&flow),
        Some(bounds),
        "hover must not move card content"
    );
    card_widget.unset_state_flags(StateFlags::PRELIGHT);
    flow.select_child(&child);
    settle();
    assert_eq!(geometry(&card_widget), rest);
    assert_eq!(
        title.compute_bounds(&flow),
        Some(bounds),
        "selection must not move card content"
    );
    grid.remove_css_class("vault-secret-sort-button-active");
    list.add_css_class("vault-secret-sort-button-active");
    let toggle_rest = geometry(&multivault);
    multivault.set_active(true);
    settle();
    assert_eq!(geometry(&multivault), toggle_rest);
    assert!(multivault.is_active());
    list.set_state_flags(StateFlags::PRELIGHT, false);
    primary.set_sensitive(false);
    snapshot(&window, "disabled-selected")?;
    list.unset_state_flags(StateFlags::PRELIGHT);
    primary.set_sensitive(true);

    popover.popup();
    settle();
    assert!(copy.grab_focus());
    copy.set_state_flags(StateFlags::FOCUS_VISIBLE, false);
    assert!(popover.is_visible());
    let menu_geometry = geometry(&menu);
    remove.set_state_flags(StateFlags::PRELIGHT, false);
    settle();
    assert_eq!(geometry(&menu), menu_geometry);
    snapshot(&window, "menu")?;
    snapshot_widget(&window, &popover, "menu-panel")?;
    copy.unset_state_flags(StateFlags::FOCUS_VISIBLE);
    remove.unset_state_flags(StateFlags::PRELIGHT);
    popover.popdown();
    let dropdown_button = dropdown
        .first_child()
        .and_downcast::<gtk4::ToggleButton>()
        .ok_or("missing native dropdown button")?;
    dropdown_button.set_active(true);
    settle();
    assert!(dropdown_button.is_active());
    dropdown.set_selected(1);
    assert_eq!(dropdown.selected(), 1);
    dropdown_button.set_active(false);
    // Use logical sizes: GTK, not application code, converts these to physical pixels.
    window.set_default_size(560, 560);
    settle();
    assert!(
        window.width() <= 560,
        "compact window unexpectedly forced wider"
    );
    for button in [&primary, &secondary, &destructive, &utility] {
        let bounds = button
            .compute_bounds(&window)
            .ok_or("missing action bounds")?;
        assert!(bounds.x() >= 0.0 && bounds.x() + bounds.width() <= window.width() as f32);
    }
    snapshot(&window, "compact")?;
    window.set_default_size(1200, 700);
    for mode in [SecretViewMode::Grid, SecretViewMode::List] {
        for class in [
            "secret-type-password",
            "secret-type-token",
            "secret-type-ssh",
            "secret-type-document",
        ] {
            let mut data = data.clone();
            data.color_class = class.to_string();
            let card = SecretCard::new(data, mode).get_widget();
            child.set_child(Some(&card));
            flow.unselect_all();
            settle();
            let header = card.first_child().ok_or("missing typed card header")?;
            let bounds = header.compute_bounds(&flow);
            let rest = geometry(&card);
            card.set_state_flags(StateFlags::PRELIGHT, false);
            flow.select_child(&child);
            settle();
            assert_eq!(geometry(&card), rest, "{mode:?} {class}: geometry changed");
            assert_eq!(
                header.compute_bounds(&flow),
                bounds,
                "{mode:?} {class}: content moved"
            );
        }
    }
    snapshot(&window, "list")?;
    window.close();
    settle();
    Ok(())
}

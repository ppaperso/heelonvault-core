use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk4::gdk::{Key, ModifierType};
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use super::events;
use crate::ui::windows::main_window::types::{FilterRuntime, SecretQuickActions};
use crate::ui::windows::main_window::{AuditFilter, SecretCategoryFilter, SecretSortMode};

pub(in crate::ui::windows::main_window) fn native_keyboard_scenario()
-> Result<(), Box<dyn std::error::Error>> {
    let window = adw::ApplicationWindow::builder()
        .default_width(700)
        .default_height(700)
        .build();
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let search = gtk4::SearchEntry::new();
    search.set_text("rechrche");
    content.append(&search);
    let inputs: Vec<gtk4::Widget> = vec![
        search.clone().upcast(),
        gtk4::Entry::new().upcast(),
        gtk4::PasswordEntry::new().upcast(),
        gtk4::TextView::new().upcast(),
        gtk4::SpinButton::with_range(0.0, 100.0, 1.0).upcast(),
        adw::EntryRow::new().upcast(),
        adw::PasswordEntryRow::new().upcast(),
        gtk4::Label::builder()
            .label("selectable")
            .selectable(true)
            .build()
            .upcast(),
        gtk4::Button::with_label("outside list").upcast(),
    ];
    for input in &inputs[1..] {
        content.append(input);
    }
    let flow = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .build();
    let first = gtk4::FlowBoxChild::new();
    let card = gtk4::Button::with_label("first");
    card.set_widget_name("keyboard-test-card");
    first.set_child(Some(&card));
    let second = gtk4::FlowBoxChild::new();
    second.set_child(Some(&gtk4::Button::with_label("second")));
    flow.insert(&first, -1);
    flow.insert(&second, -1);
    flow.select_child(&first);
    content.append(&flow);
    window.set_content(Some(&content));

    let actions_fired = Rc::new(Cell::new(0));
    let buttons: Vec<_> = (0..3).map(|_| gtk4::Button::new()).collect();
    for button in &buttons {
        let fired = Rc::clone(&actions_fired);
        button.connect_clicked(move |_| fired.set(fired.get() + 1));
    }
    let actions = Rc::new(RefCell::new(HashMap::from([(
        card.widget_name().to_string(),
        SecretQuickActions {
            copy_password: buttons[0].clone(),
            copy_login: Some(buttons[1].clone()),
            open_url: Some(buttons[2].clone()),
        },
    )])));
    events::setup_key_controller(
        &window,
        &search,
        &flow,
        actions,
        Rc::new(RefCell::new(None)),
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(0)),
        Rc::new(RefCell::new(None)),
        Rc::new(RefCell::new(Vec::new())),
    );
    let controllers = window.observe_controllers();
    let controller = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|item| item.downcast::<gtk4::EventControllerKey>().ok())
        .ok_or("missing window key controller")?;
    let press = |key: Key, modifiers: ModifierType| {
        controller.emit_by_name::<bool>("key-pressed", &[&key, &0_u32, &modifiers])
    };
    window.present();
    while glib::MainContext::default().iteration(false) {}

    for input in &inputs {
        assert!(input.grab_focus(), "cannot focus {}", input.type_().name());
        for key in [
            Key::Left,
            Key::Right,
            Key::Up,
            Key::Down,
            Key::Home,
            Key::End,
        ] {
            for modifiers in [
                ModifierType::empty(),
                ModifierType::SHIFT_MASK,
                ModifierType::CONTROL_MASK,
                ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK,
            ] {
                assert!(!press(key, modifiers), "{}: {key:?}", input.type_().name());
            }
        }
        for key in [Key::c, Key::l, Key::u, Key::a, Key::v, Key::x] {
            for modifiers in [ModifierType::CONTROL_MASK, ModifierType::META_MASK] {
                assert!(!press(key, modifiers));
            }
        }
        assert!(first.is_selected());
        assert_eq!(actions_fired.get(), 0);
    }

    for modifier in [ModifierType::CONTROL_MASK, ModifierType::META_MASK] {
        assert!(press(Key::f, modifier));
        assert_eq!(search.selection_bounds(), Some((0, 8)));
        let focus = gtk4::prelude::GtkWindowExt::focus(&window).ok_or("missing search focus")?;
        assert!(focus.is_ancestor(&search));
        assert!(
            !search.has_focus(),
            "regression requires the internal GtkText focus"
        );
    }

    assert!(card.grab_focus());
    for key in [Key::c, Key::l, Key::u] {
        assert!(press(key, ModifierType::CONTROL_MASK));
    }
    assert_eq!(actions_fired.get(), 3);
    assert!(press(Key::Right, ModifierType::empty()));
    assert!(second.is_selected());
    assert!(press(Key::Left, ModifierType::empty()));
    assert!(first.is_selected());
    for per_line in [1, 2] {
        flow.set_max_children_per_line(per_line);
        assert!(press(Key::Down, ModifierType::empty()));
        assert!(second.is_selected());
        assert!(press(Key::Up, ModifierType::empty()));
        assert!(first.is_selected());
    }

    let filter = FilterRuntime {
        meta_by_widget: Rc::default(),
        actions_by_widget: Rc::default(),
        search_text: Rc::default(),
        selected_category: Rc::new(Cell::new(SecretCategoryFilter::All)),
        selected_audit: Rc::new(Cell::new(AuditFilter::All)),
        selected_sort: Rc::new(Cell::new(SecretSortMode::Recent)),
        view_mode: Rc::new(Cell::new(crate::ui::view_preferences::SecretViewMode::Grid)),
        audit_all_count_label: gtk4::Label::new(None),
        audit_weak_count_label: gtk4::Label::new(None),
        audit_duplicate_count_label: gtk4::Label::new(None),
        total_count_label: gtk4::Label::new(None),
        non_compliant_count_label: gtk4::Label::new(None),
        incomplete_count_label: gtk4::Label::new(None),
        never_used_count_label: gtk4::Label::new(None),
        filtered_status_page: adw::StatusPage::new(),
    };
    let query = Rc::clone(&filter.search_text);
    flow.set_filter_func(move |child| match query.borrow().as_str() {
        "second" => child.index() == 1,
        "none" => false,
        _ => true,
    });
    events::setup_search_entry_handlers(&search, flow.clone(), filter.clone());
    search.grab_focus();
    search.set_text("second");
    search.emit_activate();
    assert_eq!(&*filter.search_text.borrow(), "second");
    assert!(!first.is_child_visible());
    assert!(second.is_selected());
    assert!(press(Key::Left, ModifierType::empty()));
    assert!(second.is_selected(), "filtered-out cards must be skipped");
    search.grab_focus();
    search.set_text("none");
    search.emit_activate();
    let focus = gtk4::prelude::GtkWindowExt::focus(&window).ok_or("missing search focus")?;
    assert!(focus.is_ancestor(&search));
    window.close();

    let shell =
        super::super::shell::build_content_shell(&gtk4::Frame::new(None), &gtk4::Frame::new(None));
    for language in ["en", "fr"] {
        assert!(heelonvault_core::i18n::set_language(language));
        (shell.refresh_i18n)();
        assert_eq!(
            shell.search_entry.tooltip_text().as_deref(),
            Some(heelonvault_core::tr!("main-search-keyboard-tooltip").as_str())
        );
    }
    Ok(())
}

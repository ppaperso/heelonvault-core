use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{glib, prelude::*};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub(crate) struct EditorState(Rc<State>);

struct State {
    active: Cell<bool>,
    ready: Cell<bool>,
    baseline: Cell<[u8; 32]>,
    root: glib::WeakRef<gtk4::Widget>,
    dirty_label: glib::WeakRef<gtk4::Label>,
    clear_sensitive: RefCell<Option<Box<dyn Fn()>>>,
}

impl EditorState {
    pub(crate) fn new(root: &impl IsA<gtk4::Widget>, dirty_label: &gtk4::Label) -> Self {
        Self(Rc::new(State {
            active: Cell::new(true),
            ready: Cell::new(false),
            baseline: Cell::new([0; 32]),
            root: root.as_ref().downgrade(),
            dirty_label: dirty_label.downgrade(),
            clear_sensitive: RefCell::new(None),
        }))
    }

    pub(crate) fn active(&self) -> bool {
        self.0.active.get()
    }

    pub(crate) fn ready(&self) -> bool {
        self.active() && self.0.ready.get()
    }

    pub(crate) fn set_clear_sensitive(&self, clear: impl Fn() + 'static) {
        *self.0.clear_sensitive.borrow_mut() = Some(Box::new(clear));
    }

    pub(crate) fn refresh_i18n(&self) {
        if let Some(label) = self.0.dirty_label.upgrade() {
            label.set_text(heelonvault_core::tr!("editor-unsaved").as_str());
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.ready() && self.fingerprint() != self.0.baseline.get()
    }

    fn fingerprint(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        if let Some(root) = self.0.root.upgrade() {
            visit_fields(&root, &mut |field| {
                let value = field_text(field);
                digest.update(value.len().to_le_bytes());
                digest.update(value.as_bytes());
            });
        }
        digest.finalize().into()
    }

    pub(crate) fn finish_loading(&self) {
        if !self.active() {
            return;
        }
        self.0.baseline.set(self.fingerprint());
        self.0.ready.set(true);
        if let Some(root) = self.0.root.upgrade() {
            visit_fields(&root, &mut |field| {
                let state = self.clone();
                if let Some(view) = field.downcast_ref::<gtk4::TextView>() {
                    view.buffer().connect_changed(move |_| state.refresh());
                } else if let Some(dropdown) = field.downcast_ref::<gtk4::DropDown>() {
                    dropdown.connect_selected_notify(move |_| state.refresh());
                } else if let Some(check) = field.downcast_ref::<gtk4::CheckButton>() {
                    check.connect_toggled(move |_| state.refresh());
                } else if let Some(editable) = field.dynamic_cast_ref::<gtk4::Editable>() {
                    editable.connect_changed(move |_| state.refresh());
                } else if let Some(password) = field.downcast_ref::<gtk4::PasswordEntry>() {
                    password.connect_text_notify(move |_| state.refresh());
                }
            });
        }
        self.refresh();
    }

    fn refresh(&self) {
        if let Some(label) = self.0.dirty_label.upgrade() {
            label.set_visible(self.dirty());
        }
    }

    pub(crate) fn mark_saved(&self) {
        self.0.baseline.set(self.fingerprint());
        self.refresh();
    }

    pub(crate) fn save_shortcut(&self, save: &gtk4::Button) -> gtk4::EventControllerKey {
        let shortcuts = gtk4::EventControllerKey::new();
        shortcuts.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let save = save.downgrade();
        let state = self.clone();
        shortcuts.connect_key_pressed(move |_, key, _, modifiers| {
            use gtk4::gdk::{Key, ModifierType};
            let relevant = ModifierType::CONTROL_MASK
                | ModifierType::SHIFT_MASK
                | ModifierType::ALT_MASK
                | ModifierType::SUPER_MASK;
            if matches!(key, Key::s | Key::S) && modifiers & relevant == ModifierType::CONTROL_MASK
            {
                if state.ready()
                    && let Some(save) = save.upgrade()
                    && save.is_sensitive()
                {
                    save.emit_clicked();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        shortcuts
    }

    pub(crate) fn discard(&self) {
        self.0.active.set(false);
        self.0.ready.set(false);
        if let Some(clear) = self.0.clear_sensitive.borrow_mut().take() {
            clear();
        }
        if let Some(root) = self.0.root.upgrade() {
            visit_fields(&root, &mut |field| {
                if let Some(view) = field.downcast_ref::<gtk4::TextView>() {
                    view.buffer().set_text("");
                } else if let Some(editable) = field.dynamic_cast_ref::<gtk4::Editable>() {
                    editable.set_text("");
                } else if let Some(password) = field.downcast_ref::<gtk4::PasswordEntry>() {
                    password.set_text("");
                }
            });
            root.set_sensitive(false);
        }
        self.refresh();
    }
}

fn visit_fields(widget: &gtk4::Widget, visitor: &mut impl FnMut(&gtk4::Widget)) {
    if widget.is::<gtk4::Entry>()
        || widget.is::<gtk4::PasswordEntry>()
        || widget.is::<gtk4::SpinButton>()
        || widget.is::<gtk4::TextView>()
        || widget.is::<gtk4::DropDown>()
        || widget.is::<gtk4::CheckButton>()
    {
        visitor(widget);
        return;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        visit_fields(&current, visitor);
        child = current.next_sibling();
    }
}

fn field_text(widget: &gtk4::Widget) -> glib::GString {
    if let Some(view) = widget.downcast_ref::<gtk4::TextView>() {
        let buffer = view.buffer();
        return buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
    }
    if let Some(dropdown) = widget.downcast_ref::<gtk4::DropDown>() {
        return dropdown.selected().to_string().into();
    }
    if let Some(check) = widget.downcast_ref::<gtk4::CheckButton>() {
        return if check.is_active() { "1" } else { "0" }.into();
    }
    if let Some(editable) = widget.dynamic_cast_ref::<gtk4::Editable>() {
        return editable.text();
    }
    if let Some(password) = widget.downcast_ref::<gtk4::PasswordEntry>() {
        return password.text();
    }
    glib::GString::new()
}

#[cfg(test)]
pub(crate) fn native_lifecycle_scenario() {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let label = gtk4::Label::new(None);
    let entry = gtk4::Entry::new();
    let password = gtk4::PasswordEntry::new();
    let notes = gtk4::TextView::new();
    let check = gtk4::CheckButton::new();
    let dropdown = gtk4::DropDown::from_strings(&["a", "b"]);
    let spin = gtk4::SpinButton::with_range(1.0, 30.0, 1.0);
    let save = gtk4::Button::new();
    for field in [
        entry.clone().upcast::<gtk4::Widget>(),
        password.clone().upcast(),
        notes.clone().upcast(),
        check.clone().upcast(),
        dropdown.clone().upcast(),
        spin.clone().upcast(),
        save.clone().upcast(),
    ] {
        root.append(&field);
    }
    let state = EditorState::new(&root, &label);
    let saves = Rc::new(Cell::new(0));
    let saved = Rc::clone(&saves);
    save.connect_clicked(move |_| saved.set(saved.get() + 1));
    let shortcut = state.save_shortcut(&save);
    root.add_controller(shortcut.clone());
    let ctrl_s = || {
        shortcut.emit_by_name::<bool>(
            "key-pressed",
            &[
                &gtk4::gdk::Key::s,
                &0_u32,
                &gtk4::gdk::ModifierType::CONTROL_MASK,
            ],
        )
    };
    entry.set_text("loaded");
    assert!(!state.dirty());
    ctrl_s();
    assert_eq!(saves.get(), 0, "loading cannot save");
    state.finish_loading();
    assert!(!state.dirty());
    assert!(!label.is_visible());
    let language = heelonvault_core::i18n::current_language();
    for language in ["fr", "en"] {
        assert!(heelonvault_core::i18n::set_language(language));
        state.refresh_i18n();
        assert_eq!(
            label.text().as_str(),
            heelonvault_core::tr!("editor-unsaved")
        );
    }
    assert!(heelonvault_core::i18n::set_language(&language));
    for (field, changed, original) in [
        (entry.clone().upcast::<gtk4::Widget>(), "changed", "loaded"),
        (password.clone().upcast(), "synthetic-only", ""),
        (spin.clone().upcast(), "7", "1"),
    ] {
        let editable = field.dynamic_cast_ref::<gtk4::Editable>();
        assert!(editable.is_some());
        if let Some(editable) = editable {
            editable.set_text(changed);
            assert!(state.dirty());
            assert!(label.is_visible());
            editable.set_text(original);
            assert!(!state.dirty(), "reverting clears dirty state");
        }
    }
    notes.buffer().set_text("notes");
    assert!(state.dirty());
    notes.buffer().set_text("");
    assert!(!state.dirty());
    check.set_active(true);
    assert!(state.dirty());
    check.set_active(false);
    dropdown.set_selected(1);
    assert!(state.dirty());
    dropdown.set_selected(0);
    assert!(!state.dirty());
    ctrl_s();
    assert_eq!(saves.get(), 1);
    save.set_sensitive(false);
    ctrl_s();
    assert_eq!(saves.get(), 1, "invalid or pending save cannot run");
    save.set_sensitive(true);
    entry.set_text("changed");
    state.mark_saved();
    assert!(!state.dirty());
    password.set_text("synthetic-only");
    state.discard();
    assert!(!state.active());
    assert!(!state.ready());
    assert!(entry.text().is_empty());
    assert!(password.text().is_empty());
    ctrl_s();
    assert_eq!(saves.get(), 1, "invalidated editor cannot save");
    state.finish_loading();
    assert!(
        !state.ready(),
        "obsolete initial load cannot reactivate the editor"
    );
}

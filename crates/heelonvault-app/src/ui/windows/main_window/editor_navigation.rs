use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{glib, prelude::*};
use libadwaita::{self as adw, prelude::*};

use crate::ui::dialogs::add_edit_dialog::EditorState;

#[derive(Clone)]
pub(crate) struct EditorNavigation(Rc<Navigation>);

struct Navigation {
    parent: glib::WeakRef<gtk4::Window>,
    editor: RefCell<Option<EditorState>>,
    dialog: RefCell<Option<adw::MessageDialog>>,
    busy: Rc<Cell<bool>>,
    generation: Cell<u64>,
    discard_prompt: Cell<bool>,
}

impl EditorNavigation {
    pub(crate) fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        Self(Rc::new(Navigation {
            parent: parent.as_ref().downgrade(),
            editor: RefCell::new(None),
            dialog: RefCell::new(None),
            busy: Rc::new(Cell::new(false)),
            generation: Cell::new(0),
            discard_prompt: Cell::new(false),
        }))
    }

    pub(crate) fn busy(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.0.busy)
    }

    pub(crate) fn refresh_i18n(&self) {
        if let Some(editor) = self.0.editor.borrow().as_ref() {
            editor.refresh_i18n();
        }
        if let Some(dialog) = self.0.dialog.borrow().as_ref() {
            if self.0.discard_prompt.get() {
                dialog.set_heading(Some(heelonvault_core::tr!("editor-discard-title").as_str()));
                dialog.set_body(heelonvault_core::tr!("editor-discard-body").as_str());
                dialog.set_response_label(
                    "keep",
                    heelonvault_core::tr!("editor-keep-editing").as_str(),
                );
                dialog.set_response_label(
                    "discard",
                    heelonvault_core::tr!("editor-discard").as_str(),
                );
            } else {
                dialog.set_heading(Some(
                    heelonvault_core::tr!("main-close-busy-title").as_str(),
                ));
                dialog.set_body(heelonvault_core::tr!("main-close-busy-body").as_str());
                dialog.set_response_label(
                    "close",
                    heelonvault_core::tr!("editor-keep-editing").as_str(),
                );
            }
        }
    }

    pub(crate) fn install(&self, editor: EditorState) {
        self.force_discard();
        *self.0.editor.borrow_mut() = Some(editor);
    }

    pub(crate) fn force_discard(&self) {
        self.0
            .generation
            .set(self.0.generation.get().wrapping_add(1));
        if let Some(editor) = self.0.editor.borrow_mut().take() {
            editor.discard();
        }
        let dialog = self.0.dialog.borrow_mut().take();
        if let Some(dialog) = dialog {
            dialog.close();
        }
    }

    pub(crate) fn request(&self, action: impl FnOnce() + 'static) {
        self.request_if(|| true, action);
    }

    fn request_if(
        &self,
        available: impl FnOnce() -> bool + 'static,
        action: impl FnOnce() + 'static,
    ) {
        let Some(parent) = self.0.parent.upgrade() else {
            return;
        };
        if let Some(dialog) = self.0.dialog.borrow().as_ref() {
            dialog.present();
            return;
        }
        if self.0.busy.get() {
            let dialog = adw::MessageDialog::new(
                Some(&parent),
                Some(heelonvault_core::tr!("main-close-busy-title").as_str()),
                Some(heelonvault_core::tr!("main-close-busy-body").as_str()),
            );
            dialog.add_response(
                "close",
                heelonvault_core::tr!("editor-keep-editing").as_str(),
            );
            dialog.set_modal(true);
            dialog.set_close_response("close");
            let weak = Rc::downgrade(&self.0);
            let generation = self.0.generation.get();
            dialog.connect_response(None, move |_, _| {
                if let Some(inner) = weak.upgrade()
                    && inner.generation.get() == generation
                {
                    inner.dialog.borrow_mut().take();
                }
            });
            self.0.discard_prompt.set(false);
            *self.0.dialog.borrow_mut() = Some(dialog.clone());
            dialog.present();
            return;
        }
        let dirty = self
            .0
            .editor
            .borrow()
            .as_ref()
            .is_some_and(EditorState::dirty);
        if !dirty {
            if available() {
                self.force_discard();
                action();
            }
            return;
        }
        let dialog = adw::MessageDialog::new(
            Some(&parent),
            Some(heelonvault_core::tr!("editor-discard-title").as_str()),
            Some(heelonvault_core::tr!("editor-discard-body").as_str()),
        );
        dialog.add_responses(&[
            (
                "keep",
                heelonvault_core::tr!("editor-keep-editing").as_str(),
            ),
            ("discard", heelonvault_core::tr!("editor-discard").as_str()),
        ]);
        dialog.set_modal(true);
        dialog.set_default_response(Some("keep"));
        dialog.set_close_response("keep");
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let action = RefCell::new(Some((available, action)));
        let weak = Rc::downgrade(&self.0);
        let generation = self.0.generation.get();
        dialog.connect_response(None, move |_, response| {
            let action = action.borrow_mut().take();
            let Some(inner) = weak.upgrade() else { return };
            if inner.generation.get() != generation {
                return;
            }
            inner.dialog.borrow_mut().take();
            if response == "discard"
                && let Some((available, action)) = action
            {
                if !available() {
                    tracing::info!(
                        "editor navigation cancelled: target changed during confirmation"
                    );
                    return;
                }
                let navigation = EditorNavigation(inner);
                navigation.force_discard();
                action();
            }
        });
        *self.0.dialog.borrow_mut() = Some(dialog.clone());
        self.0.discard_prompt.set(true);
        dialog.present();
    }

    #[cfg(test)]
    pub(crate) fn native_navigation_scenario() -> Result<(), Box<dyn std::error::Error>> {
        let window = gtk4::Window::builder()
            .default_width(700)
            .default_height(700)
            .build();
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let form = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let entry = gtk4::Entry::new();
        form.append(&entry);
        content.append(&form);
        let list = gtk4::ListBox::new();
        list.append(&gtk4::Label::new(Some("one")));
        list.append(&gtk4::Label::new(Some("two")));
        content.append(&list);
        window.set_child(Some(&content));
        window.present();
        let navigation = EditorNavigation::new(&window);
        let label = gtk4::Label::new(None);
        let state = EditorState::new(&form, &label);
        state.finish_loading();
        navigation.install(state.clone());
        let calls = Rc::new(Cell::new(0));
        let action = {
            let calls = Rc::clone(&calls);
            move || calls.set(calls.get() + 1)
        };
        entry.set_text("unsaved");
        navigation.request(action.clone());
        assert_eq!(calls.get(), 0);
        let dialog = navigation
            .0
            .dialog
            .borrow()
            .clone()
            .ok_or("missing discard prompt")?;
        assert_eq!(dialog.close_response().as_str(), "keep");
        assert_eq!(dialog.default_response().as_deref(), Some("keep"));
        assert!(dialog.is_modal());
        navigation.request(action.clone());
        assert_eq!(
            navigation.0.dialog.borrow().as_ref(),
            Some(&dialog),
            "one prompt only"
        );
        dialog.response("keep");
        assert_eq!(calls.get(), 0);
        assert!(state.dirty());
        assert_eq!(entry.text().as_str(), "unsaved");

        list.select_row(list.row_at_index(0).as_ref());
        let selected = Rc::new(Cell::new(0));
        let selected_for_action = Rc::clone(&selected);
        navigation.guard_selection(&list, Rc::new(Cell::new(false)), move |row| {
            selected_for_action.set(row.index());
        });
        list.select_row(list.row_at_index(1).as_ref());
        assert_eq!(list.selected_row().map(|row| row.index()), Some(0));
        assert_eq!(
            selected.get(),
            0,
            "selection side effects wait for approval"
        );
        let dialog = navigation
            .0
            .dialog
            .borrow()
            .clone()
            .ok_or("missing selection prompt")?;
        dialog.response("keep");
        assert_eq!(list.selected_row().map(|row| row.index()), Some(0));
        list.select_row(list.row_at_index(1).as_ref());
        let dialog = navigation
            .0
            .dialog
            .borrow()
            .clone()
            .ok_or("missing stale selection prompt")?;
        let removed = list.row_at_index(1).ok_or("missing second row")?;
        list.remove(&removed);
        dialog.response("discard");
        assert!(
            state.dirty(),
            "a vanished target must not discard the draft"
        );
        assert_eq!(selected.get(), 0);
        list.append(&removed);
        list.select_row(list.row_at_index(1).as_ref());
        let dialog = navigation
            .0
            .dialog
            .borrow()
            .clone()
            .ok_or("missing selection prompt")?;
        dialog.response("discard");
        assert_eq!(selected.get(), 1);
        assert_eq!(list.selected_row().map(|row| row.index()), Some(1));
        assert!(!state.active());
        assert!(entry.text().is_empty());

        form.set_sensitive(true);
        let state = EditorState::new(&form, &label);
        state.finish_loading();
        navigation.install(state.clone());
        navigation.request(action.clone());
        assert_eq!(calls.get(), 1, "clean navigation is immediate");
        assert!(!state.active());
        form.set_sensitive(true);
        let state = EditorState::new(&form, &label);
        state.finish_loading();
        navigation.install(state.clone());
        entry.set_text("pending draft");
        navigation.request(action.clone());
        let dialog = navigation
            .0
            .dialog
            .borrow()
            .clone()
            .ok_or("missing lock prompt")?;
        navigation.refresh_i18n();
        navigation.force_discard();
        dialog.response("discard");
        assert_eq!(calls.get(), 1, "a stale prompt cannot navigate after lock");
        assert!(!state.active());
        assert!(entry.text().is_empty());
        navigation.busy().set(true);
        navigation.request(action);
        assert_eq!(calls.get(), 1, "pending mutations block navigation");
        navigation.force_discard();
        navigation.busy().set(false);
        window.destroy();
        Ok(())
    }

    pub(crate) fn guard_selection(
        &self,
        list: &gtk4::ListBox,
        suppressed: Rc<Cell<bool>>,
        apply: impl Fn(&gtk4::ListBoxRow) + 'static,
    ) {
        let previous = Rc::new(RefCell::new(list.selected_row().map(|row| row.downgrade())));
        let apply = Rc::new(apply);
        let navigation = self.clone();
        let restoring = Rc::new(Cell::new(false));
        list.connect_row_selected(move |list, row| {
            if restoring.get() {
                return;
            }
            if suppressed.get() {
                *previous.borrow_mut() = row.map(|row| row.downgrade());
                return;
            }
            let Some(row) = row else { return };
            let row = row.clone();
            restoring.set(true);
            list.select_row(
                previous
                    .borrow()
                    .as_ref()
                    .and_then(glib::WeakRef::upgrade)
                    .as_ref(),
            );
            restoring.set(false);
            let list = list.clone();
            let previous = Rc::clone(&previous);
            let restoring = Rc::clone(&restoring);
            let apply = Rc::clone(&apply);
            let target = row.clone();
            let target_list = list.downgrade();
            navigation.request_if(
                move || {
                    target_list
                        .upgrade()
                        .is_some_and(|list| target.parent().as_ref() == Some(list.upcast_ref()))
                },
                move || {
                    restoring.set(true);
                    list.select_row(Some(&row));
                    *previous.borrow_mut() = Some(row.downgrade());
                    restoring.set(false);
                    apply(&row);
                },
            );
        });
    }
}

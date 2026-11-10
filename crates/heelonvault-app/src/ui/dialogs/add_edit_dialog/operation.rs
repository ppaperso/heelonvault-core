use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;

use gtk4::{glib, prelude::*};

#[derive(Clone)]
pub(super) struct EditOperation {
    pub(super) busy: Rc<Cell<bool>>,
    critical_operations: Rc<Cell<u32>>,
    editor: glib::WeakRef<gtk4::ScrolledWindow>,
    delete: glib::WeakRef<gtk4::Button>,
    spinner: glib::WeakRef<gtk4::Spinner>,
    refresh_save: Rc<dyn Fn()>,
}

impl EditOperation {
    pub(super) fn new(
        busy: Rc<Cell<bool>>,
        critical_operations: Rc<Cell<u32>>,
        editor: &gtk4::ScrolledWindow,
        delete: &gtk4::Button,
        spinner: &gtk4::Spinner,
        refresh_save: Rc<dyn Fn()>,
    ) -> Self {
        Self {
            busy,
            critical_operations,
            editor: editor.downgrade(),
            delete: delete.downgrade(),
            spinner: spinner.downgrade(),
            refresh_save,
        }
    }

    pub(super) fn begin(&self) -> Option<PendingEditOperation> {
        if self.busy.replace(true) {
            return None;
        }
        self.critical_operations
            .set(self.critical_operations.get().saturating_add(1));
        if let Some(editor) = self.editor.upgrade() {
            editor.set_sensitive(false);
        }
        if let Some(delete) = self.delete.upgrade() {
            delete.set_sensitive(false);
        }
        if let Some(spinner) = self.spinner.upgrade() {
            spinner.set_visible(true);
            spinner.set_spinning(true);
        }
        (self.refresh_save)();
        Some(PendingEditOperation(self.clone()))
    }
}

pub(super) struct PendingEditOperation(EditOperation);

impl PendingEditOperation {
    pub(super) async fn wait<T>(self, result: impl Future<Output = T>) -> T {
        let result = result.await;
        drop(self);
        result
    }
}

impl Drop for PendingEditOperation {
    fn drop(&mut self) {
        self.0.busy.set(false);
        self.0
            .critical_operations
            .set(self.0.critical_operations.get().saturating_sub(1));
        if let Some(editor) = self.0.editor.upgrade() {
            editor.set_sensitive(true);
        }
        if let Some(delete) = self.0.delete.upgrade() {
            delete.set_sensitive(true);
        }
        if let Some(spinner) = self.0.spinner.upgrade() {
            spinner.set_spinning(false);
            spinner.set_visible(false);
        }
        (self.0.refresh_save)();
    }
}

#[cfg(test)]
pub(crate) fn native_operation_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let editor = gtk4::ScrolledWindow::new();
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let save = gtk4::Button::new();
    let delete = gtk4::Button::new();
    let spinner = gtk4::Spinner::new();
    spinner.set_visible(false);
    content.append(&save);
    content.append(&delete);
    content.append(&spinner);
    editor.set_child(Some(&content));
    let busy = Rc::new(Cell::new(false));
    let critical_operations = Rc::new(Cell::new(0));
    let valid = Rc::new(Cell::new(true));
    let refresh_save: Rc<dyn Fn()> = {
        let save = save.downgrade();
        let busy = Rc::clone(&busy);
        let valid = Rc::clone(&valid);
        Rc::new(move || {
            if let Some(save) = save.upgrade() {
                save.set_sensitive(!busy.get() && valid.get());
            }
        })
    };
    let operation = EditOperation::new(
        Rc::clone(&busy),
        Rc::clone(&critical_operations),
        &editor,
        &delete,
        &spinner,
        Rc::clone(&refresh_save),
    );

    for outcome in [Some(Ok(())), Some(Err(())), None] {
        valid.set(true);
        let pending = operation.begin().ok_or("operation did not start")?;
        assert!(
            operation.begin().is_none(),
            "duplicate operation was accepted"
        );
        assert!(!editor.is_sensitive());
        assert_eq!(critical_operations.get(), 1);
        assert!(!save.is_sensitive());
        assert!(!delete.is_sensitive());
        assert!(spinner.is_spinning());
        valid.set(false);
        refresh_save();
        assert!(!save.is_sensitive());

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let finished = Rc::new(Cell::new(false));
        let finished_for_task = Rc::clone(&finished);
        glib::MainContext::default().spawn_local(async move {
            let _ = pending.wait(receiver).await;
            finished_for_task.set(true);
        });
        let context = glib::MainContext::default();
        context.iteration(false);
        assert!(!finished.get(), "operation completed before its result");
        assert!(busy.get());
        assert!(!editor.is_sensitive());
        match outcome {
            Some(result) => sender
                .send(result)
                .map_err(|_| "result receiver was lost")?,
            None => drop(sender),
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !finished.get() {
            if std::time::Instant::now() >= deadline {
                return Err("operation did not release its UI".into());
            }
            context.iteration(false);
        }
        assert!(!busy.get());
        assert_eq!(critical_operations.get(), 0);
        assert!(editor.is_sensitive());
        assert!(delete.is_sensitive());
        assert!(!spinner.is_spinning());
        assert!(!spinner.is_visible());
        assert!(!save.is_sensitive(), "invalid form became submittable");
    }
    let pending = operation.begin().ok_or("operation did not restart")?;
    drop(pending);
    assert!(!busy.get(), "cancelled task kept the editor busy");
    assert_eq!(critical_operations.get(), 0);
    assert!(editor.is_sensitive());
    Ok(())
}

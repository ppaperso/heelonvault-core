use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;
use tokio::runtime::Runtime;
use uuid::Uuid;

use super::{AddEditDialog, DialogMode};
use heelonvault_core::services::{secret_service::SecretService, vault_service::VaultService};

fn find(widget: &gtk4::Widget, class: &str) -> Option<gtk4::Widget> {
    if widget.has_css_class(class) {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = find(&current, class) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

fn wait_until(complete: impl Fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !complete() {
        if Instant::now() >= deadline {
            return Err("editor operation did not complete".into());
        }
        gtk4::glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

pub(crate) fn native_inline_scenario<TSecret, TVault>(
    runtime: &Runtime,
    secrets: Arc<TSecret>,
    vaults: Arc<TVault>,
    user_id: Uuid,
    secret_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>>
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let second_vault = runtime.block_on(vaults.create_vault(
        user_id,
        "Second editor test vault",
        secrecy::SecretBox::new(Box::new(vec![1; 32])),
    ))?;
    for mode in [
        DialogMode::Create,
        DialogMode::CreateInVault(second_vault.id),
        DialogMode::CreateInVault(Uuid::new_v4()),
    ] {
        let editor = AddEditDialog::build_inline(
            runtime.handle().clone(),
            Arc::clone(&secrets),
            Arc::clone(&vaults),
            user_id,
            vec![1; 32],
            false,
            mode,
            Rc::new(Cell::new(false)),
            Rc::new(Cell::new(0)),
            || {},
            |_| {},
            |_| {},
        );
        let root = editor.container.upcast_ref::<gtk4::Widget>();
        let error = find(root, "login-error").ok_or("missing load error")?;
        wait_until(|| editor.state.ready() || error.is_visible())?;
        if matches!(mode, DialogMode::CreateInVault(id) if id != second_vault.id) {
            assert!(
                !editor.state.ready(),
                "unavailable target must not fall back to another vault"
            );
            assert!(error.is_visible());
        } else {
            assert!(editor.state.ready());
            assert!(!editor.state.dirty());
            if matches!(mode, DialogMode::CreateInVault(_)) {
                let pills =
                    find(root, "add-edit-vault-pills-row").ok_or("missing vault choices")?;
                let mut child = pills.first_child();
                let mut selected = None;
                while let Some(pill) = child {
                    if let Some(check) = pill.downcast_ref::<gtk4::CheckButton>()
                        && check.is_active()
                    {
                        selected = check.label();
                    }
                    child = pill.next_sibling();
                }
                assert_eq!(selected.as_deref(), Some(second_vault.name.as_str()));
            }
        }
        editor.state.discard();
    }
    for (key, invalidate) in [(2, false), (1, false), (1, true)] {
        let busy = Rc::new(Cell::new(false));
        let critical = Rc::new(Cell::new(0));
        let saved = Rc::new(Cell::new(0));
        let cancelled = Rc::new(Cell::new(0));
        let on_saved = Rc::clone(&saved);
        let on_cancel = Rc::clone(&cancelled);
        let editor = AddEditDialog::build_inline(
            runtime.handle().clone(),
            Arc::clone(&secrets),
            Arc::clone(&vaults),
            user_id,
            vec![key; 32],
            false,
            DialogMode::Edit(secret_id),
            Rc::clone(&busy),
            Rc::clone(&critical),
            move || on_cancel.set(on_cancel.get() + 1),
            move |_| on_saved.set(on_saved.get() + 1),
            |_| {},
        );
        assert!(!editor.state.ready());
        let root = editor.container.upcast_ref::<gtk4::Widget>();
        let title = find(root, "dialog-title-entry")
            .and_then(|widget| widget.downcast::<gtk4::Entry>().ok())
            .ok_or("missing title input")?;
        let save = find(root, "primary-pill")
            .and_then(|widget| widget.downcast::<gtk4::Button>().ok())
            .ok_or("missing save button")?;
        assert!(!save.is_sensitive(), "initial loading disables saving");
        wait_until(|| editor.state.ready())?;
        assert!(
            !editor.state.dirty(),
            "loaded fields are the clean baseline"
        );
        assert!(save.is_sensitive());
        title.set_text(if invalidate {
            "late save"
        } else {
            "edited title"
        });
        assert!(editor.state.dirty());
        save.emit_clicked();
        assert!(busy.get());
        assert_eq!(critical.get(), 1);
        save.emit_clicked();
        assert_eq!(critical.get(), 1, "duplicate saves share the same guard");
        if invalidate {
            editor.state.discard();
        }
        wait_until(|| !busy.get())?;
        assert_eq!(critical.get(), 0);
        if invalidate {
            assert_eq!(saved.get(), 0);
            assert_eq!(cancelled.get(), 0, "late completion cannot navigate");
            assert!(title.text().is_empty());
            assert!(!editor.state.ready());
        } else if key == 2 {
            assert_eq!(saved.get(), 0);
            assert_eq!(cancelled.get(), 0);
            assert!(editor.state.dirty(), "a failed save retains the draft");
            assert!(save.is_sensitive(), "a failed save permits retry");
            assert!(find(root, "login-error").is_some_and(|label| label.is_visible()));
        } else {
            assert_eq!(saved.get(), 1);
            assert_eq!(cancelled.get(), 1);
            assert!(
                !editor.state.dirty(),
                "successful save clears dirty state first"
            );
        }
        editor.state.discard();
    }
    let editor = AddEditDialog::build_inline(
        runtime.handle().clone(),
        secrets,
        vaults,
        user_id,
        vec![1; 32],
        true,
        DialogMode::Edit(secret_id),
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(0)),
        || {},
        |_| {},
        |_| {},
    );
    editor.state.discard();
    let until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < until {
        gtk4::glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !editor.state.ready(),
        "late initial load cannot repopulate a locked editor"
    );
    let title = find(editor.container.upcast_ref(), "dialog-title-entry")
        .and_then(|widget| widget.downcast::<gtk4::Entry>().ok())
        .ok_or("missing invalidated title")?;
    assert!(title.text().is_empty());
    Ok(())
}

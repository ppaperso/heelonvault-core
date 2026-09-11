//! Inline secret editor page.
//!
//! The editor is not a modal dialog: it lives as a `secret_editor_view` page of the
//! main stack, so the sidebar stays visible while creating or editing a secret.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk4::prelude::*;
use libadwaita as adw;
use tokio::runtime::Handle;
use tracing::info;
use uuid::Uuid;

use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

use crate::ui::dialogs::add_edit_dialog::{AddEditDialog, DialogMode};
use crate::ui::messages;

/// Everything the editor launcher needs to build an inline editor on demand.
pub struct EditorDeps<TSecret, TVault> {
    pub runtime_handle: Handle,
    pub secret_service: Arc<TSecret>,
    pub vault_service: Arc<TVault>,
    pub user_id: Uuid,
    pub session_master_key: Rc<RefCell<Vec<u8>>>,
    pub show_passwords_in_edit: Rc<Cell<bool>>,
    pub main_stack: gtk4::Stack,
    pub editor_host: gtk4::Box,
    pub toast_overlay: adw::ToastOverlay,
    pub refresh_entries: Rc<dyn Fn()>,
}

/// Create the editor host container and register it as the `secret_editor_view` stack page.
pub fn build_editor_page(main_stack: &gtk4::Stack) -> gtk4::Box {
    let editor_host = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .build();
    main_stack.add_titled(
        &editor_host,
        Some("secret_editor_view"),
        heelonvault_core::tr!("main-stack-editor").as_str(),
    );
    editor_host
}

/// Build the callback that swaps the editor page content and brings it to the front.
///
/// Returns a no-op when the session is locked: the master key is required to decrypt the
/// secret being edited, so opening the editor without it would expose an empty form.
pub fn build_editor_launcher<TSecret, TVault>(
    deps: EditorDeps<TSecret, TVault>,
) -> Rc<dyn Fn(DialogMode)>
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    Rc::new(move |mode| {
        let Some(master_key) =
            super::super::MainWindow::snapshot_session_master_key(&deps.session_master_key)
        else {
            info!("secret editor blocked: session is locked");
            return;
        };

        while let Some(child) = deps.editor_host.first_child() {
            deps.editor_host.remove(&child);
        }

        let stack_for_cancel = deps.main_stack.clone();
        let inline_view = AddEditDialog::build_inline(
            deps.runtime_handle.clone(),
            Arc::clone(&deps.secret_service),
            Arc::clone(&deps.vault_service),
            deps.user_id,
            master_key,
            deps.show_passwords_in_edit.get(),
            mode,
            move || {
                stack_for_cancel.set_visible_child_name("entries_view");
            },
            {
                let refresh_after_save = Rc::clone(&deps.refresh_entries);
                let toast_overlay = deps.toast_overlay.clone();
                move |saved_title: String| {
                    refresh_after_save();
                    let message = messages::toast_secret_saved(saved_title.as_str());
                    toast_overlay.add_toast(adw::Toast::new(message.as_str()));
                }
            },
            {
                let refresh_after_delete = Rc::clone(&deps.refresh_entries);
                let toast_overlay = deps.toast_overlay.clone();
                move |deleted_title: String| {
                    refresh_after_delete();
                    let message = messages::toast_secret_deleted(deleted_title.as_str());
                    toast_overlay.add_toast(adw::Toast::new(message.as_str()));
                }
            },
        );

        deps.editor_host.append(&inline_view.container);
        deps.main_stack.set_visible_child_name("secret_editor_view");
    })
}

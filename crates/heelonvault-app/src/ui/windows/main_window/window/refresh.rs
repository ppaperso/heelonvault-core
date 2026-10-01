//! Secret list refresh.
//!
//! A single parameterized reload backs both the normal (active vault) refresh and the
//! multivault "search everywhere" reload — they only differ by the `all_vaults` flag.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use libadwaita as adw;
use tokio::runtime::Handle;
use uuid::Uuid;
use zeroize::Zeroizing;

use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

use crate::ui::dialogs::add_edit_dialog::DialogMode;
use crate::ui::windows::main_window::secret_flow;
use crate::ui::windows::main_window::types::FilterRuntime;

pub struct SecretRefreshDeps<TSecret, TVault> {
    pub parent_window: adw::ApplicationWindow,
    pub runtime_handle: Handle,
    pub secret_service: Arc<TSecret>,
    pub vault_service: Arc<TVault>,
    pub user_id: Uuid,
    pub session_master_key: Rc<RefCell<Vec<u8>>>,
    pub active_vault_id: Rc<RefCell<Option<Uuid>>>,
    pub secret_flow: gtk4::FlowBox,
    pub stack: gtk4::Stack,
    pub empty_title: gtk4::Label,
    pub empty_copy: gtk4::Label,
    pub toast_overlay: adw::ToastOverlay,
    pub filter_runtime: FilterRuntime,
    pub editor_launcher: Rc<RefCell<Option<Rc<dyn Fn(DialogMode)>>>>,
}

/// Callbacks driving the secret list.
pub struct SecretListCallbacks {
    /// Reload from the database. The `bool` selects cross-vault search: `false` reloads the
    /// active vault, `true` reloads every vault the user can read.
    pub reload: Rc<dyn Fn(bool)>,
    /// Rebuild the widgets from the last load (no database access, no decryption); returns
    /// `false` when nothing has been loaded yet.
    pub rerender: Rc<dyn Fn() -> bool>,
}

/// Build the secret list callbacks, sharing one context between reload and re-render.
pub fn build_secret_reload<TSecret, TVault>(
    deps: SecretRefreshDeps<TSecret, TVault>,
) -> SecretListCallbacks
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let ctx = Rc::new(secret_flow::SecretFlowContext {
        parent_window: deps.parent_window,
        runtime_handle: deps.runtime_handle,
        secret_service: deps.secret_service,
        vault_service: deps.vault_service,
        user_id: deps.user_id,
        session_master_key: deps.session_master_key,
        secret_flow: deps.secret_flow,
        stack: deps.stack,
        empty_title: deps.empty_title,
        empty_copy: deps.empty_copy,
        active_vault_id: deps.active_vault_id,
        toast_overlay: deps.toast_overlay,
        filter_runtime: deps.filter_runtime,
        editor_launcher: deps.editor_launcher,
        last_loaded: RefCell::new(None),
    });

    let ctx_for_reload = Rc::clone(&ctx);
    let reload: Rc<dyn Fn(bool)> = Rc::new(move |all_vaults: bool| {
        let ctx = &ctx_for_reload;
        let Some(master_key) =
            super::super::MainWindow::snapshot_session_master_key(&ctx.session_master_key)
        else {
            *ctx.last_loaded.borrow_mut() = None;
            ctx.empty_title
                .set_text(heelonvault_core::tr!("main-session-locked-title").as_str());
            ctx.empty_copy
                .set_text(heelonvault_core::tr!("main-session-locked-description").as_str());
            ctx.stack.set_visible_child_name("empty");
            return;
        };

        secret_flow::refresh_secret_flow(ctx, Zeroizing::new(master_key), all_vaults);
    });

    let rerender: Rc<dyn Fn() -> bool> = Rc::new(move || secret_flow::rerender_secret_flow(&ctx));

    SecretListCallbacks { reload, rerender }
}

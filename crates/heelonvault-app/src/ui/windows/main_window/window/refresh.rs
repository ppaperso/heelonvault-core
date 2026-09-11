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

use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

use crate::ui::dialogs::add_edit_dialog::DialogMode;
use crate::ui::windows::main_window::secret_flow;
use crate::ui::windows::main_window::types::FilterRuntime;

pub struct SecretRefreshDeps<TSecret, TVault> {
    pub application: adw::Application,
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

/// Build the secret list reload callback.
///
/// The `bool` argument selects cross-vault search: `false` reloads the active vault,
/// `true` reloads every vault the user can read.
pub fn build_secret_reload<TSecret, TVault>(
    deps: SecretRefreshDeps<TSecret, TVault>,
) -> Rc<dyn Fn(bool)>
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    Rc::new(move |all_vaults: bool| {
        let Some(master_key) =
            super::super::MainWindow::snapshot_session_master_key(&deps.session_master_key)
        else {
            deps.empty_title
                .set_text(heelonvault_core::tr!("main-session-locked-title").as_str());
            deps.empty_copy
                .set_text(heelonvault_core::tr!("main-session-locked-description").as_str());
            deps.stack.set_visible_child_name("empty");
            return;
        };

        secret_flow::refresh_secret_flow(
            deps.application.clone(),
            deps.parent_window.clone(),
            deps.runtime_handle.clone(),
            Arc::clone(&deps.secret_service),
            Arc::clone(&deps.vault_service),
            deps.user_id,
            master_key,
            deps.secret_flow.clone(),
            deps.stack.clone(),
            deps.empty_title.clone(),
            deps.empty_copy.clone(),
            Rc::clone(&deps.active_vault_id),
            deps.toast_overlay.clone(),
            deps.filter_runtime.clone(),
            Rc::clone(&deps.editor_launcher),
            all_vaults,
        );
    })
}

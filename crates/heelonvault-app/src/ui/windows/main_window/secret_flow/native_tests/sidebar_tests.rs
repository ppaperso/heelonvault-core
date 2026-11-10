use super::*;
use crate::ui::windows::main_window::window::vault_list::{
    VaultListDeps, build_refresh_vault_sections, find_vault_row,
};

pub(super) fn scenario<TSecret, TVault>(
    runtime: &tokio::runtime::Runtime,
    ctx: &SecretFlowContext<ControlledSecret<TSecret>, ControlledVault<TVault>>,
    controlled: &Arc<ControlledVault<TVault>>,
    first: &Vault,
    second: &Vault,
) -> Result<(), Box<dyn std::error::Error>>
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let sidebar = crate::ui::windows::main_window::sidebar::build_sidebar_panel();
    let generation = Rc::new(LoadGeneration::default());
    let busy = Rc::new(Cell::new(false));
    let pending = Rc::new(Cell::new(false));
    let creating = Rc::new(Cell::new(false));
    let callback = Rc::new(RefCell::new(None));
    let completed = Rc::new(Cell::new(0));
    let completed_for_refresh = Rc::clone(&completed);
    let refresh = build_refresh_vault_sections(VaultListDeps {
        generation: Rc::clone(&generation),
        window: ctx.parent_window.clone(),
        toast_overlay: ctx.toast_overlay.clone(),
        runtime_handle: runtime.handle().clone(),
        secret_service: Arc::clone(&ctx.secret_service),
        vault_service: Arc::clone(controlled),
        user_id: ctx.user_id,
        session_master_key: Rc::clone(&ctx.session_master_key),
        active_vault_id: Rc::clone(&ctx.active_vault_id),
        my_vaults_list: sidebar.my_vaults_list.clone(),
        shared_vaults_title: sidebar.shared_vaults_title,
        shared_vaults_list: sidebar.shared_vaults_list,
        selection_sync: Rc::new(Cell::new(false)),
        default_vault_creation: Rc::clone(&creating),
        refresh_in_progress: Rc::clone(&busy),
        refresh_pending: Rc::clone(&pending),
        refresh_secrets: Rc::new(move || {
            completed_for_refresh.set(completed_for_refresh.get() + 1)
        }),
        refresh_after_mutation: Rc::clone(&callback),
    });
    *callback.borrow_mut() = Some(Rc::clone(&refresh));

    let (release_old, mut entered_old) =
        ControlledVault::<TVault>::queue_gate(&controlled.owned_gates)?;
    refresh();
    wait(|| entered_old.try_recv().is_ok())?;
    let (release_new, mut entered_new) =
        ControlledVault::<TVault>::queue_gate(&controlled.owned_gates)?;
    refresh();
    assert!(pending.get());
    release_old
        .send(Ok(vec![first.clone()]))
        .map_err(|_| "old sidebar gate lost")?;
    wait(|| entered_new.try_recv().is_ok())?;
    assert!(sidebar.my_vaults_list.first_child().is_none());
    assert_eq!(completed.get(), 0);
    release_new
        .send(Ok(vec![second.clone()]))
        .map_err(|_| "new sidebar gate lost")?;
    wait(|| !busy.get())?;
    assert!(find_vault_row(&sidebar.my_vaults_list, second.id).is_some());
    assert!(find_vault_row(&sidebar.my_vaults_list, first.id).is_none());
    assert_eq!(completed.get(), 1);

    let (release, mut entered) = ControlledVault::<TVault>::queue_gate(&controlled.owned_gates)?;
    refresh();
    wait(|| entered.try_recv().is_ok())?;
    ctx.session_master_key.borrow_mut().clear();
    generation.invalidate();
    busy.set(false);
    pending.set(false);
    creating.set(false);
    *ctx.session_master_key.borrow_mut() = vec![1; 32];
    release
        .send(Ok(vec![first.clone()]))
        .map_err(|_| "locked sidebar gate lost")?;
    wait(|| Rc::strong_count(&generation) == 2)?;
    assert!(sidebar.my_vaults_list.first_child().is_none());
    assert_eq!(completed.get(), 1);

    let (release, _) = ControlledVault::<TVault>::queue_gate(&controlled.owned_gates)?;
    refresh();
    release
        .send(Err(AppError::Storage(
            "synthetic sidebar failure".to_string(),
        )))
        .map_err(|_| "failed sidebar gate lost")?;
    wait(|| !busy.get())?;
    assert_eq!(completed.get(), 2);
    assert!(!creating.get());
    let (release, _) = ControlledVault::<TVault>::queue_gate(&controlled.owned_gates)?;
    refresh();
    release
        .send(Ok(vec![first.clone()]))
        .map_err(|_| "sidebar retry gate lost")?;
    wait(|| !busy.get())?;
    assert!(find_vault_row(&sidebar.my_vaults_list, first.id).is_some());
    assert_eq!(completed.get(), 3);
    callback.borrow_mut().take();
    Ok(())
}

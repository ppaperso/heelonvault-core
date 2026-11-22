use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use heelonvault_core::models::{AccessibleVault, SecretType, Vault};
use libadwaita::prelude::*;

use super::*;
use crate::ui::view_preferences::SecretViewMode;
mod controlled_secret;
mod sidebar_tests;
use controlled_secret::ControlledSecret;

type VaultResult = Result<Vec<Vault>, AppError>;
type Gate = (
    tokio::sync::oneshot::Receiver<VaultResult>,
    tokio::sync::oneshot::Sender<()>,
);

struct ControlledVault<T> {
    inner: Arc<T>,
    gates: Mutex<VecDeque<Gate>>,
    owned_gates: Mutex<VecDeque<Gate>>,
    fault: Mutex<Option<(Uuid, bool)>>,
}

impl<T> ControlledVault<T> {
    fn gate(
        &self,
    ) -> Result<
        (
            tokio::sync::oneshot::Sender<VaultResult>,
            tokio::sync::oneshot::Receiver<()>,
        ),
        Box<dyn std::error::Error>,
    > {
        Self::queue_gate(&self.gates)
    }

    fn queue_gate(
        gates: &Mutex<VecDeque<Gate>>,
    ) -> Result<
        (
            tokio::sync::oneshot::Sender<VaultResult>,
            tokio::sync::oneshot::Receiver<()>,
        ),
        Box<dyn std::error::Error>,
    > {
        let (release, receiver) = tokio::sync::oneshot::channel();
        let (entered, waiting) = tokio::sync::oneshot::channel();
        gates
            .lock()
            .map_err(|_| "test gate poisoned")?
            .push_back((receiver, entered));
        Ok((release, waiting))
    }

    fn fails(&self, id: Uuid, opening: bool) -> Result<bool, AppError> {
        self.fault
            .lock()
            .map(|fault| *fault == Some((id, opening)))
            .map_err(|_| AppError::Storage("test fault poisoned".to_string()))
    }
}

impl<T: VaultService + Send + Sync> VaultService for ControlledVault<T> {
    async fn list_user_vaults(&self, user: Uuid) -> VaultResult {
        let gate = self
            .gates
            .lock()
            .map_err(|_| AppError::Storage("test gate poisoned".to_string()))?
            .pop_front();
        if let Some((receiver, entered)) = gate {
            let _ = entered.send(());
            return receiver
                .await
                .map_err(|_| AppError::Storage("test gate closed".to_string()))?;
        }
        self.inner.list_user_vaults(user).await
    }
    async fn get_vault_access_for_user(
        &self,
        user: Uuid,
        id: Uuid,
    ) -> Result<Option<AccessibleVault>, AppError> {
        if self.fails(id, false)? {
            return Ok(None);
        }
        self.inner.get_vault_access_for_user(user, id).await
    }
    async fn open_vault_for_user(
        &self,
        user: Uuid,
        id: Uuid,
        key: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        if self.fails(id, true)? {
            return Err(AppError::Storage(
                "synthetic vault opening failure".to_string(),
            ));
        }
        self.inner.open_vault_for_user(user, id, key).await
    }
    async fn create_vault(
        &self,
        user: Uuid,
        name: &str,
        key: SecretBox<Vec<u8>>,
    ) -> Result<Vault, AppError> {
        self.inner.create_vault(user, name, key).await
    }
    async fn open_vault(
        &self,
        id: Uuid,
        key: SecretBox<Vec<u8>>,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        self.inner.open_vault(id, key).await
    }
    async fn list_owned_vaults(&self, user: Uuid) -> VaultResult {
        let gate = self
            .owned_gates
            .lock()
            .map_err(|_| AppError::Storage("test gate poisoned".to_string()))?
            .pop_front();
        if let Some((receiver, entered)) = gate {
            let _ = entered.send(());
            return receiver
                .await
                .map_err(|_| AppError::Storage("test gate closed".to_string()))?;
        }
        self.inner.list_owned_vaults(user).await
    }
    async fn list_shared_vaults(&self, user: Uuid) -> VaultResult {
        self.inner.list_shared_vaults(user).await
    }
    async fn list_shared_vault_access(&self, user: Uuid) -> Result<Vec<AccessibleVault>, AppError> {
        self.inner.list_shared_vault_access(user).await
    }
    async fn is_vault_shared_with_others(&self, user: Uuid, id: Uuid) -> Result<bool, AppError> {
        self.inner.is_vault_shared_with_others(user, id).await
    }
    async fn delete_vault(&self, user: Uuid, id: Uuid) -> Result<(), AppError> {
        self.inner.delete_vault(user, id).await
    }
    async fn update_vault_sort_order(
        &self,
        user: Uuid,
        id: Uuid,
        order: i64,
    ) -> Result<(), AppError> {
        self.inner.update_vault_sort_order(user, id, order).await
    }
}

fn wait(complete: impl FnMut() -> bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut complete = complete;
    let deadline = Instant::now() + Duration::from_secs(8);
    while !complete() {
        if Instant::now() >= deadline {
            return Err("list fixture timed out".into());
        }
        glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

pub(crate) fn scenario<TSecret, TVault>(
    runtime: &tokio::runtime::Runtime,
    secrets: Arc<TSecret>,
    vaults: Arc<TVault>,
    user: Uuid,
) -> Result<(), Box<dyn std::error::Error>>
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let (first, second) = runtime.block_on(async {
        let first = vaults
            .create_vault(user, "Load A", SecretBox::new(Box::new(vec![1; 32])))
            .await?;
        let second = vaults
            .create_vault(user, "Load B", SecretBox::new(Box::new(vec![1; 32])))
            .await?;
        for vault in [&first, &second] {
            let key = vaults
                .open_vault_for_user(user, vault.id, SecretBox::new(Box::new(vec![1; 32])))
                .await?;
            secrets
                .create_secret(
                    vault.id,
                    SecretType::ApiToken,
                    Some(vault.name.clone()),
                    None,
                    None,
                    None,
                    SecretBox::new(Box::new(b"synthetic-token".to_vec())),
                    key,
                )
                .await?;
        }
        Ok::<_, AppError>((first, second))
    })?;
    let controlled = Arc::new(ControlledVault {
        inner: vaults,
        gates: Mutex::new(VecDeque::new()),
        owned_gates: Mutex::new(VecDeque::new()),
        fault: Mutex::new(None),
    });
    let panel = super::super::center::build_center_panel(SecretViewMode::Grid);
    let sidebar = super::super::sidebar::build_sidebar_panel();
    let filters = FilterRuntime {
        load_ready: Rc::new(Cell::new(false)),
        filter_summary: panel.filter_summary,
        meta_by_widget: Rc::new(RefCell::new(HashMap::new())),
        actions_by_widget: Rc::new(RefCell::new(HashMap::new())),
        search_text: Rc::new(RefCell::new(String::new())),
        selected_category: Rc::new(Cell::new(super::super::SecretCategoryFilter::All)),
        selected_audit: Rc::new(Cell::new(super::super::AuditFilter::All)),
        selected_sort: Rc::new(Cell::new(super::super::SecretSortMode::Recent)),
        view_mode: Rc::new(Cell::new(SecretViewMode::Grid)),
        audit_all_count_label: sidebar.audit_all_badge,
        audit_weak_count_label: sidebar.audit_weak_badge,
        audit_duplicate_count_label: sidebar.audit_duplicate_badge,
        total_count_label: panel.status_total_badge.clone(),
        non_compliant_count_label: panel.status_non_compliant_badge,
        incomplete_count_label: panel.status_incomplete_badge,
        never_used_count_label: panel.status_never_used_badge,
        filtered_status_page: panel.filtered_status_page,
    };
    super::super::window::core::install_flow_filter_and_sort(&panel.secret_flow, &filters);
    let window = adw::ApplicationWindow::builder()
        .default_width(820)
        .default_height(800)
        .build();
    window.set_content(Some(&panel.frame));
    let ctx = Rc::new(SecretFlowContext {
        parent_window: window.clone(),
        runtime_handle: runtime.handle().clone(),
        secret_service: Arc::new(ControlledSecret {
            inner: secrets,
            fail_list: Mutex::new(None),
        }),
        vault_service: Arc::clone(&controlled),
        user_id: user,
        session_master_key: Rc::new(RefCell::new(vec![1; 32])),
        secret_flow: panel.secret_flow,
        stack: panel.stack,
        empty_title: panel.empty_title,
        empty_copy: panel.empty_copy,
        active_vault_id: Rc::new(RefCell::new(Some(first.id))),
        toast_overlay: adw::ToastOverlay::new(),
        filter_runtime: filters,
        editor_launcher: Rc::new(RefCell::new(None)),
        last_loaded: RefCell::new(None),
        generation: LoadGeneration::default(),
        load_state: Cell::new(ListLoadState::Idle),
        load_status: panel.load_status,
        retry_load: panel.retry_load,
    });
    let load = |global| refresh_secret_flow(&ctx, Zeroizing::new(vec![1; 32]), global);
    let settled = || Rc::strong_count(&ctx) == 1;
    window.present();

    // Let B finish before A: neither the old result nor its failure can replace B.
    for late_error in [false, true] {
        *ctx.active_vault_id.borrow_mut() = Some(first.id);
        let (release_old, mut entered) = controlled.gate()?;
        load(false);
        wait(|| entered.try_recv().is_ok())?;
        let (release_global, _) = controlled.gate()?;
        load(true);
        release_global
            .send(Ok(vec![first.clone(), second.clone()]))
            .map_err(|_| "latest global gate lost")?;
        wait(|| {
            ctx.last_loaded
                .borrow()
                .as_ref()
                .is_some_and(|loaded| loaded.is_global)
        })?;
        release_old
            .send(Err(AppError::Storage(
                "obsolete single-vault failure".to_string(),
            )))
            .map_err(|_| "old single gate lost")?;
        wait(settled)?;
        assert!(matches!(
            ctx.load_state.get(),
            ListLoadState::Ready { global: true, .. }
        ));
        assert_eq!(ctx.filter_runtime.total_count_label.text(), "2");
        let (release, mut entered) = controlled.gate()?;
        load(late_error);
        wait(|| entered.try_recv().is_ok())?;
        assert_eq!(ctx.filter_runtime.total_count_label.text(), "-");
        assert!(ctx.filter_runtime.actions_by_widget.borrow().is_empty());
        assert!(!rerender_secret_flow(&ctx));
        *ctx.active_vault_id.borrow_mut() = Some(second.id);
        load(false);
        wait(|| ctx.last_loaded.borrow().is_some())?;
        let result = if late_error {
            Err(AppError::Storage("synthetic late failure".to_string()))
        } else {
            Ok(vec![first.clone()])
        };
        release.send(result).map_err(|_| "gate release lost")?;
        wait(settled)?;
        assert_eq!(*ctx.active_vault_id.borrow(), Some(second.id));
        assert_eq!(
            ctx.last_loaded
                .borrow()
                .as_ref()
                .map(|loaded| loaded.rows[0].title.as_str().to_owned()),
            Some(second.name.clone())
        );
    }
    let (release, mut entered) = controlled.gate()?;
    load(true);
    wait(|| entered.try_recv().is_ok())?;
    ctx.session_master_key.borrow_mut().clear();
    invalidate_secret_flow(&ctx);
    *ctx.session_master_key.borrow_mut() = vec![1; 32];
    release
        .send(Ok(vec![first.clone()]))
        .map_err(|_| "lock gate lost")?;
    wait(settled)?;
    assert!(ctx.last_loaded.borrow().is_none());
    assert_eq!(ctx.load_state.get(), ListLoadState::Locked);
    assert!(
        !rerender_secret_flow(&ctx),
        "unlock alone must not resurrect old results"
    );

    for opening in [false, true] {
        *controlled.fault.lock().map_err(|_| "fault poisoned")? = Some((first.id, opening));
        let (release, _) = controlled.gate()?;
        load(true);
        release
            .send(Ok(vec![first.clone(), second.clone()]))
            .map_err(|_| "partial gate lost")?;
        wait(settled)?;
        assert_eq!(
            ctx.load_state.get(),
            ListLoadState::Ready {
                global: true,
                coverage: Coverage {
                    vaults: 2,
                    loaded_vaults: 1,
                    failed_vaults: 1,
                    failed_items: 0
                },
            }
        );
        assert_eq!(ctx.filter_runtime.total_count_label.text(), "1");
        assert!(ctx.retry_load.is_visible());
        assert!(rerender_secret_flow(&ctx));
        assert!(ctx.load_status.has_css_class("warning"));
        ctx.filter_runtime
            .selected_category
            .set(super::super::SecretCategoryFilter::Password);
        search_filter::apply_filters(&ctx.secret_flow, &ctx.filter_runtime);
        assert!(ctx.filter_runtime.filtered_status_page.is_visible());
        assert!(
            ctx.filter_runtime
                .filter_summary
                .text()
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains("0/1")
        );
        assert!(ctx.load_status.has_css_class("warning"));
        ctx.filter_runtime
            .selected_category
            .set(super::super::SecretCategoryFilter::All);
        ctx.filter_runtime.view_mode.set(SecretViewMode::List);
        assert!(rerender_secret_flow(&ctx));
        assert!(
            ctx.filter_runtime
                .filter_summary
                .text()
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains("1/1")
        );
        ctx.filter_runtime.view_mode.set(SecretViewMode::Grid);
    }
    let (release, _) = controlled.gate()?;
    load(true);
    release
        .send(Ok(vec![first.clone()]))
        .map_err(|_| "failure gate lost")?;
    wait(settled)?;
    assert_eq!(ctx.load_state.get(), ListLoadState::Failed);
    assert!(ctx.last_loaded.borrow().is_none());
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "-");
    *controlled.fault.lock().map_err(|_| "fault poisoned")? = None;
    *ctx.secret_service
        .fail_list
        .lock()
        .map_err(|_| "list fault poisoned")? = Some(first.id);
    let (release, _) = controlled.gate()?;
    load(true);
    release
        .send(Ok(vec![first.clone(), second.clone()]))
        .map_err(|_| "listing gate lost")?;
    wait(settled)?;
    assert!(
        matches!(ctx.load_state.get(), ListLoadState::Ready { coverage, .. } if coverage.failed_vaults == 1)
    );
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "1");
    *ctx.secret_service
        .fail_list
        .lock()
        .map_err(|_| "list fault poisoned")? = None;
    let (release, _) = controlled.gate()?;
    load(true);
    release
        .send(Ok(vec![first.clone(), second.clone()]))
        .map_err(|_| "recovery gate lost")?;
    wait(settled)?;
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "2");
    assert!(!ctx.retry_load.is_visible());
    assert!(!ctx.load_status.has_css_class("warning"));
    let (release, _) = controlled.gate()?;
    load(true);
    release
        .send(Ok(Vec::new()))
        .map_err(|_| "empty gate lost")?;
    wait(settled)?;
    assert_eq!(
        ctx.load_state.get(),
        ListLoadState::Ready {
            global: true,
            coverage: Coverage::default()
        }
    );
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "0");
    assert!(!ctx.retry_load.is_visible());
    *ctx.active_vault_id.borrow_mut() = None;
    let (release, _) = controlled.gate()?;
    load(false);
    release
        .send(Ok(Vec::new()))
        .map_err(|_| "no-selection gate lost")?;
    wait(settled)?;
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "0");
    assert_eq!(
        ctx.load_status.text().as_str(),
        heelonvault_core::tr!("main-no-vault-selected-title")
    );
    *ctx.active_vault_id.borrow_mut() = Some(first.id);
    let (release, _) = controlled.gate()?;
    load(false);
    release
        .send(Ok(Vec::new()))
        .map_err(|_| "disappeared selection gate lost")?;
    wait(settled)?;
    assert_eq!(ctx.load_state.get(), ListLoadState::Failed);
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "-");
    let empty_vault = runtime.block_on(controlled.create_vault(
        user,
        "Empty fixture",
        SecretBox::new(Box::new(vec![1; 32])),
    ))?;
    *controlled.fault.lock().map_err(|_| "fault poisoned")? = Some((first.id, true));
    let (release, _) = controlled.gate()?;
    load(true);
    release
        .send(Ok(vec![first.clone(), empty_vault.clone()]))
        .map_err(|_| "partial empty gate lost")?;
    wait(settled)?;
    assert!(
        matches!(ctx.load_state.get(), ListLoadState::Ready { coverage, .. } if coverage.partial())
    );
    assert_eq!(
        ctx.empty_title.text().as_str(),
        heelonvault_core::tr!("main-list-partial-empty-title")
    );
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "0");
    *controlled.fault.lock().map_err(|_| "fault poisoned")? = None;
    let empty_key = runtime.block_on(controlled.open_vault_for_user(
        user,
        empty_vault.id,
        SecretBox::new(Box::new(vec![1; 32])),
    ))?;
    runtime.block_on(ctx.secret_service.create_secret(
        empty_vault.id,
        SecretType::Password,
        Some("Invalid UTF-8 fixture".to_string()),
        None,
        None,
        None,
        SecretBox::new(Box::new(vec![0xff])),
        empty_key,
    ))?;
    *ctx.active_vault_id.borrow_mut() = Some(empty_vault.id);
    load(false);
    wait(settled)?;
    assert!(
        matches!(ctx.load_state.get(), ListLoadState::Ready { coverage, .. } if coverage.failed_items == 1)
    );
    assert_eq!(
        ctx.empty_title.text().as_str(),
        heelonvault_core::tr!("main-list-partial-empty-title")
    );
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "0");
    runtime.block_on(ctx.secret_service.create_secret(
        first.id,
        SecretType::ApiToken,
        Some("Unreadable fixture".to_string()),
        None,
        None,
        None,
        SecretBox::new(Box::new(b"synthetic-only".to_vec())),
        SecretBox::new(Box::new(vec![9; 32])),
    ))?;
    *ctx.active_vault_id.borrow_mut() = Some(first.id);
    load(false);
    wait(settled)?;
    assert_eq!(
        ctx.load_state.get(),
        ListLoadState::Ready {
            global: false,
            coverage: Coverage {
                vaults: 1,
                loaded_vaults: 1,
                failed_vaults: 0,
                failed_items: 1
            },
        }
    );
    assert!(
        ctx.filter_runtime
            .filter_summary
            .text()
            .replace(['\u{2068}', '\u{2069}'], "")
            .contains("1/1")
    );
    wait(|| ctx.load_status.width() > 0)?;
    assert!(
        window.width() <= 820,
        "list content must fit the existing compact qualification width"
    );
    assert_eq!(ctx.filter_runtime.total_count_label.text(), "1");
    assert!(ctx.retry_load.is_visible());
    let language = heelonvault_core::i18n::current_language();
    for lang in ["fr", "en"] {
        assert!(heelonvault_core::i18n::set_language(lang));
        refresh_load_i18n(&ctx);
        assert_eq!(
            ctx.load_status.text().as_str(),
            ctx.load_state.get().message()
        );
        assert!(window.measure(gtk4::Orientation::Horizontal, -1).0 <= 820);
        let load_row = ctx.load_status.parent().ok_or("missing load row")?;
        assert!(load_row.measure(gtk4::Orientation::Horizontal, -1).0 <= 560);
        assert!(
            ctx.filter_runtime
                .filter_summary
                .measure(gtk4::Orientation::Horizontal, -1)
                .0
                <= 560
        );
    }
    assert!(heelonvault_core::i18n::set_language(&language));
    invalidate_secret_flow(&ctx);
    sidebar_tests::scenario(runtime, &ctx, &controlled, &first, &second)?;
    window.close();
    Ok(())
}

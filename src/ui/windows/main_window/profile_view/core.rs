use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_profile_view<
    TUser,
    TTotp,
    TPolicy,
    TBackup,
    TBackupApp,
    TImport,
    TSecret,
    TVault,
>(
    window: adw::ApplicationWindow,
    runtime_handle: Handle,
    user_service: Arc<TUser>,
    totp_service: Arc<TTotp>,
    auth_policy_service: Arc<TPolicy>,
    backup_service: Arc<TBackup>,
    backup_app_service: Arc<TBackupApp>,
    import_service: Arc<TImport>,
    secret_service: Arc<TSecret>,
    vault_service: Arc<TVault>,
    database_path: PathBuf,
    user_id: Uuid,
    _is_admin: bool,
    profile_badge: gtk4::MenuButton,
    critical_ops_in_flight: Rc<Cell<u32>>,
    auto_lock_timeout_secs: Rc<Cell<u64>>,
    auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: Rc<Cell<bool>>,
    on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    show_passwords_in_edit_pref: Rc<Cell<bool>>,
    on_import_completed_refresh: Rc<dyn Fn()>,
    on_language_changed: Rc<dyn Fn()>,
) -> ProfileViewWidgets
where
    TUser: UserService + Send + Sync + 'static,
    TTotp: TotpService + Send + Sync + 'static,
    TPolicy: AuthPolicyService + Send + Sync + 'static,
    TBackup: BackupService + Send + Sync + 'static,
    TBackupApp: BackupApplicationService + Send + Sync + 'static,
    TImport: ImportService + Send + Sync + 'static,
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    include!("parts/build_profile_view_body.inc")
}

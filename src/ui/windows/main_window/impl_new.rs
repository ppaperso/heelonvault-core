use super::*;

impl MainWindow {
    pub fn new<
        TSecret,
        TVault,
        TUser,
        TAdmin,
        TTeam,
        TTotp,
        TPolicy,
        TBackup,
        TBackupApp,
        TImport,
    >(
        application: &adw::Application,
        runtime_handle: Handle,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        user_service: Arc<TUser>,
        admin_service: Arc<TAdmin>,
        team_service: Arc<TTeam>,
        totp_service: Arc<TTotp>,
        auth_policy_service: Arc<TPolicy>,
        backup_service: Arc<TBackup>,
        backup_app_service: Arc<TBackupApp>,
        import_service: Arc<TImport>,
        audit_service: Arc<crate::services::audit_service::AuditService>,
        license_service: Arc<LicenseService>,
        database_pool: SqlitePool,
        database_path: PathBuf,
        admin_user_id: Uuid,
        admin_master_key: Vec<u8>,
        connected_identity_label: String,
        license_badge_text: String,
        is_admin: bool,
    ) -> Self
    where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
        TUser: UserService + Send + Sync + 'static,
        TAdmin: AdminService + Send + Sync + 'static,
        TTeam: TeamService + Send + Sync + 'static,
        TTotp: TotpService + Send + Sync + 'static,
        TPolicy: AuthPolicyService + Send + Sync + 'static,
        TBackup: BackupService + Send + Sync + 'static,
        TBackupApp: BackupApplicationService + Send + Sync + 'static,
        TImport: ImportService + Send + Sync + 'static,
    {
        include!("impl_core_parts/new_body.inc")
    }
}

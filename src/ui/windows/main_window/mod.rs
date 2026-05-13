#![allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::redundant_allocation
)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, NaiveDateTime, Timelike, Utc};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;
use secrecy::SecretBox;
use sqlx::{Row, SqlitePool};
use tokio::runtime::Handle;
use tracing::{info, warn};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::models::LicenseTier;
use crate::services::admin_service::AdminService;
use crate::services::audit_report_service::{AuditReportService, ReportError};
use crate::services::auth_policy_service::AuthPolicyService;
use crate::services::backup_application_service::BackupApplicationService;
use crate::services::backup_service::BackupService;
use crate::services::import_service::ImportService;
use crate::services::license_service::LicenseService;
use crate::services::login_history_service::list_recent_logins;
use crate::services::secret_service::SecretService;
use crate::services::team_service::TeamService;
use crate::services::totp_service::TotpService;
use crate::services::user_service::UserService;
use crate::services::vault_service::VaultService;
use crate::ui::dialogs::add_edit_dialog::{AddEditDialog, DialogMode};
use crate::ui::dialogs::manage_teams_dialog::ManageTeamsDialog;
use crate::ui::dialogs::manage_users_dialog::ManageUsersDialog;
use crate::ui::dialogs::recovery_key_export_dialog::{
    ExportRunner, RecoveryKeyExportDialog, RecoveryKeyExportDialogDeps,
};
use crate::ui::dialogs::trash_dialog::TrashDialog;
use crate::ui::messages;
use crate::ui::window_sizing;

mod auto_lock;
mod center;
mod certification;
mod header;
mod impl_new;
mod impl_post;
mod impl_preamble;
mod profile_view;
mod search_filter;
mod secret_flow;
mod shell;
mod sidebar;
mod types;

use self::types::{FilterRuntime, SecretFilterMeta, SecretRowView};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecretCategoryFilter {
    All,
    Password,
    ApiToken,
    SshKey,
    SecureDocument,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuditFilter {
    All,
    Weak,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecretSortMode {
    Recent,
    Title,
    Risk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SecretKind {
    Password,
    ApiToken,
    SshKey,
    SecureDocument,
}

pub struct MainWindow {
    window: adw::ApplicationWindow,
    secret_flow: gtk4::FlowBox,
    refresh_entries: Rc<dyn Fn()>,
    auto_lock_timeout_secs: Rc<Cell<u64>>,
    auto_lock_source: Rc<RefCell<Option<glib::SourceId>>>,
    auto_lock_armed: Rc<Cell<bool>>,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    on_auto_lock: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    on_logout: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    #[allow(dead_code)]
    audit_service: Rc<Arc<crate::services::audit_service::AuditService>>,
}

struct CenterPanelWidgets {
    frame: gtk4::Frame,
    main_stack: gtk4::Stack,
    stack: gtk4::Stack,
    list_page: gtk4::Box,
    empty_state: gtk4::Box,
    secret_flow: gtk4::FlowBox,
    filtered_status_page: adw::StatusPage,
    status_total_chip: gtk4::Box,
    status_total_badge: gtk4::Label,
    status_non_compliant_chip: gtk4::Box,
    status_non_compliant_badge: gtk4::Label,
    sort_recent_button: gtk4::Button,
    sort_title_button: gtk4::Button,
    sort_risk_button: gtk4::Button,
    empty_title: gtk4::Label,
    empty_copy: gtk4::Label,
}

struct ProfileViewWidgets {
    container: gtk4::ScrolledWindow,
    back_button: gtk4::Button,
}

struct SidebarWidgets {
    frame: gtk4::Frame,
    my_vaults_title: gtk4::Label,
    create_vault_button: gtk4::Button,
    my_vaults_list: gtk4::ListBox,
    shared_vaults_title: gtk4::Label,
    shared_vaults_list: gtk4::ListBox,
    category_list: gtk4::ListBox,
    audit_list: gtk4::ListBox,
    audit_title: gtk4::Label,
    categories_title: gtk4::Label,
    account_title: gtk4::Label,
    audit_all_label: gtk4::Label,
    audit_weak_label: gtk4::Label,
    audit_duplicate_label: gtk4::Label,
    category_all_label: gtk4::Label,
    category_passwords_label: gtk4::Label,
    category_api_tokens_label: gtk4::Label,
    category_ssh_keys_label: gtk4::Label,
    category_documents_label: gtk4::Label,
    audit_all_badge: gtk4::Label,
    audit_weak_badge: gtk4::Label,
    audit_duplicate_badge: gtk4::Label,
    certification_menu_button: gtk4::MenuButton,
    profile_security_label: gtk4::Label,
    profile_security_button: gtk4::Button,
    teams_label: gtk4::Label,
    teams_button: gtk4::Button,
    administration_label: gtk4::Label,
    administration_button: gtk4::Button,
}

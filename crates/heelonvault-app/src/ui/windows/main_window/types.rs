use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use libadwaita as adw;
use uuid::Uuid;

use super::{AuditFilter, SecretCategoryFilter, SecretKind, SecretSortMode};

#[allow(dead_code)]
#[derive(Clone)]
pub(super) struct SecretQuickActions {
    #[allow(dead_code)]
    pub(super) copy_password: gtk4::Button,
    #[allow(dead_code)]
    pub(super) copy_login: Option<gtk4::Button>,
    #[allow(dead_code)]
    pub(super) open_url: Option<gtk4::Button>,
}

#[allow(dead_code)]
#[derive(Clone)]
pub(super) struct SecretFilterMeta {
    pub(super) searchable_text: String,
    pub(super) title_text: String,
    pub(super) login_text: String,
    pub(super) email_text: String,
    pub(super) url_text: String,
    pub(super) notes_text: String,
    pub(super) category_text: String,
    pub(super) tags_text: String,
    pub(super) type_text: String,
    pub(super) vault_name_text: String,
    #[allow(dead_code)]
    pub(super) kind: SecretKind,
    #[allow(dead_code)]
    pub(super) original_rank: usize,
    pub(super) is_weak: bool,
    pub(super) is_duplicate: bool,
    pub(super) is_health: bool,
}

#[allow(dead_code)]
#[derive(Clone)]
pub(super) struct FilterRuntime {
    #[allow(dead_code)]
    pub(super) meta_by_widget: Rc<RefCell<HashMap<String, SecretFilterMeta>>>,
    #[allow(dead_code)]
    pub(super) actions_by_widget: Rc<RefCell<HashMap<String, SecretQuickActions>>>,
    #[allow(dead_code)]
    pub(super) search_text: Rc<RefCell<String>>,
    #[allow(dead_code)]
    pub(super) selected_category: Rc<Cell<SecretCategoryFilter>>,
    #[allow(dead_code)]
    pub(super) selected_audit: Rc<Cell<AuditFilter>>,
    #[allow(dead_code)]
    pub(super) selected_sort: Rc<Cell<SecretSortMode>>,
    pub(super) audit_all_count_label: gtk4::Label,
    pub(super) audit_weak_count_label: gtk4::Label,
    pub(super) audit_duplicate_count_label: gtk4::Label,
    pub(super) total_count_label: gtk4::Label,
    pub(super) non_compliant_count_label: gtk4::Label,
    pub(super) filtered_status_page: adw::StatusPage,
}

#[allow(dead_code)]
pub(super) struct SecretRowView {
    pub(super) secret_id: Uuid,
    pub(super) icon_name: String,
    pub(super) type_label: String,
    pub(super) title: String,
    pub(super) created_at: String,
    pub(super) login: String,
    pub(super) email: String,
    pub(super) url: String,
    pub(super) notes: String,
    pub(super) category: String,
    pub(super) tags: String,
    pub(super) secret_value: String,
    pub(super) kind: SecretKind,
    pub(super) color_class: String,
    pub(super) health: String,
    pub(super) is_health_access: bool,
    pub(super) usage_count: u32,
    pub(super) vault_name: String,
    /// (is_shared, can_write, can_admin) — used to set button sensitivity
    pub(super) vault_access: (bool, bool, bool),
}

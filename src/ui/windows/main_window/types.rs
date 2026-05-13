use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use libadwaita as adw;
use uuid::Uuid;

use super::{AuditFilter, SecretCategoryFilter, SecretKind, SecretSortMode};

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
    pub(super) kind: SecretKind,
    pub(super) original_rank: usize,
    pub(super) is_weak: bool,
    pub(super) is_duplicate: bool,
}

#[derive(Clone)]
pub(super) struct FilterRuntime {
    pub(super) meta_by_widget: Rc<RefCell<HashMap<String, SecretFilterMeta>>>,
    pub(super) search_text: Rc<RefCell<String>>,
    pub(super) selected_category: Rc<Cell<SecretCategoryFilter>>,
    pub(super) selected_audit: Rc<Cell<AuditFilter>>,
    pub(super) selected_sort: Rc<Cell<SecretSortMode>>,
    pub(super) audit_all_count_label: gtk4::Label,
    pub(super) audit_weak_count_label: gtk4::Label,
    pub(super) audit_duplicate_count_label: gtk4::Label,
    pub(super) total_count_label: gtk4::Label,
    pub(super) non_compliant_count_label: gtk4::Label,
    pub(super) filtered_status_page: adw::StatusPage,
}

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
    pub(super) usage_count: u32,
}

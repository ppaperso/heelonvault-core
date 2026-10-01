use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use secrecy::{ExposeSecret, SecretBox};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::ui::dialogs::add_edit_dialog::DialogMode;
use crate::ui::messages;
use crate::ui::sensitive_clipboard;
use crate::ui::widgets::secret_card::{SecretCard, SecretRowData, strength_label};
use crate::ui::windows::main_window::types::SecretQuickActions;
use heelonvault_core::errors::AppError;
use heelonvault_core::models::SecretItem;
use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

use super::{FilterRuntime, SecretFilterMeta, SecretKind, SecretRowView, search_filter};

/// Strength verdict, language-independent: the label is only translated for display.
pub(super) fn is_weak_password(secret_value: &str) -> bool {
    if secret_value.len() >= 12 {
        let has_uppercase = secret_value.chars().any(|c| c.is_uppercase());
        let has_lowercase = secret_value.chars().any(|c| c.is_lowercase());
        let has_digit = secret_value.chars().any(|c| c.is_numeric());
        let has_special = secret_value.chars().any(|c| !c.is_alphanumeric());
        let complexity = [has_uppercase, has_lowercase, has_digit, has_special]
            .iter()
            .filter(|&&v| v)
            .count();
        if complexity >= 3 {
            return false;
        }
    }
    true
}

/// SHA-256 of a secret value, used only to detect reused passwords during a list load.
type SecretFingerprint = [u8; 32];

/// Fingerprint of a decrypted value; `None` for an empty value (never a duplicate).
fn secret_fingerprint(secret_value: &[u8]) -> Option<SecretFingerprint> {
    if secret_value.is_empty() {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(secret_value);
    Some(hasher.finalize().into())
}

/// `true` for every row whose fingerprint appears more than once in the batch.
fn duplicate_flags(fingerprints: &[Option<SecretFingerprint>]) -> Vec<bool> {
    let mut counts: HashMap<SecretFingerprint, usize> = HashMap::new();
    for fingerprint in fingerprints.iter().flatten() {
        *counts.entry(*fingerprint).or_insert(0) += 1;
    }
    fingerprints
        .iter()
        .map(|fingerprint| {
            fingerprint.is_some_and(|value| counts.get(&value).copied().unwrap_or(0) > 1)
        })
        .collect()
}

/// Mark reused passwords, then drop the fingerprints: they never leave the loader thread.
fn finalize_rows(analyzed: Vec<(SecretRowView, Option<SecretFingerprint>)>) -> Vec<SecretRowView> {
    let (mut rows, fingerprints): (Vec<_>, Vec<_>) = analyzed.into_iter().unzip();
    for (row, is_duplicate) in rows.iter_mut().zip(duplicate_flags(&fingerprints)) {
        row.is_duplicate = is_duplicate;
    }
    rows
}

/// Decode a single `SecretItem` into a `SecretRowView` plus its duplicate fingerprint.
///
/// The secret value is decrypted only to derive the strength verdict and the fingerprint,
/// then wiped (it stays inside the `SecretBox` returned by the service). The row itself never
/// carries the value: copying decrypts it again on demand.
/// Returns `None` if the secret cannot be opened (wrong key, corrupt, etc.).
async fn build_secret_row<TSecret>(
    item: SecretItem,
    secret_service: &Arc<TSecret>,
    vault_key: &secrecy::SecretBox<Vec<u8>>,
    vault_name: String,
    vault_access: (bool, bool, bool),
) -> Option<(SecretRowView, Option<SecretFingerprint>)>
where
    TSecret: SecretService + Send + Sync + 'static,
{
    let secret_result = secret_service
        .get_secret(
            item.id,
            SecretBox::new(Box::new(vault_key.expose_secret().clone())),
        )
        .await;
    let (has_secret, is_weak, fingerprint) = match &secret_result {
        Ok(secret) => {
            let bytes = secret.secret_value.expose_secret().as_slice();
            let text = std::str::from_utf8(bytes).unwrap_or_default();
            (
                !bytes.is_empty(),
                is_weak_password(text),
                secret_fingerprint(bytes),
            )
        }
        Err(_) => (false, true, None),
    };
    drop(secret_result);

    let (login, email, url, notes, category, has_health_access_marker) =
        match item.metadata_json.as_deref() {
            Some(raw) => match serde_json::from_str::<Value>(raw) {
                Ok(value) => {
                    let login = value
                        .get("login")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let email = value
                        .get("email")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let url = value
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let notes = value
                        .get("notes")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let category = value
                        .get("category")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let has_health_access_marker = value
                        .get("health_access")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    (login, email, url, notes, category, has_health_access_marker)
                }
                Err(_) => (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    false,
                ),
            },
            None => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                false,
            ),
        };

    let (icon_name, type_label_text) = match item.secret_type {
        heelonvault_core::models::SecretType::Password => (
            "dialog-password-symbolic",
            heelonvault_core::tr!("secret-type-password"),
        ),
        heelonvault_core::models::SecretType::ApiToken => (
            "dialog-key-symbolic",
            heelonvault_core::tr!("secret-type-api-token"),
        ),
        heelonvault_core::models::SecretType::SshKey => (
            "network-wired-symbolic",
            heelonvault_core::tr!("secret-type-ssh-key"),
        ),
        heelonvault_core::models::SecretType::SecureDocument => (
            "folder-documents-symbolic",
            heelonvault_core::tr!("secret-type-secure-document"),
        ),
    };
    let (color_class, kind) = match item.secret_type {
        heelonvault_core::models::SecretType::Password => {
            ("secret-type-password", SecretKind::Password)
        }
        heelonvault_core::models::SecretType::ApiToken => {
            ("secret-type-token", SecretKind::ApiToken)
        }
        heelonvault_core::models::SecretType::SshKey => ("secret-type-ssh", SecretKind::SshKey),
        heelonvault_core::models::SecretType::SecureDocument => {
            ("secret-type-document", SecretKind::SecureDocument)
        }
    };

    let title = item.title.unwrap_or_else(|| type_label_text.clone());
    let created_at = item
        .created_at
        .unwrap_or_else(|| heelonvault_core::tr!("login-history-unavailable"));
    let tags = item.tags.clone().unwrap_or_default();
    let is_health_access = has_health_access_marker
        || search_filter::classify_health_access(
            title.as_str(),
            login.as_str(),
            url.as_str(),
            notes.as_str(),
            category.as_str(),
            tags.as_str(),
            type_label_text.as_str(),
        );

    let row = SecretRowView {
        secret_id: item.id,
        vault_id: item.vault_id,
        icon_name: icon_name.to_string(),
        type_label: type_label_text.to_string(),
        title,
        created_at,
        login,
        email,
        url,
        notes,
        category,
        tags,
        has_secret,
        kind,
        color_class: color_class.to_string(),
        is_weak,
        is_duplicate: false,
        is_health_access,
        usage_count: item.usage_count,
        vault_name,
        vault_access,
    };
    Some((row, fingerprint))
}

/// Everything the secret list needs to load, render and act on secrets.
pub(super) struct SecretFlowContext<TSecret, TVault> {
    pub(super) parent_window: adw::ApplicationWindow,
    pub(super) runtime_handle: Handle,
    pub(super) secret_service: Arc<TSecret>,
    pub(super) vault_service: Arc<TVault>,
    pub(super) user_id: Uuid,
    /// Live session key (emptied on lock); only snapshotted for the time of one operation.
    pub(super) session_master_key: Rc<RefCell<Vec<u8>>>,
    pub(super) secret_flow: gtk4::FlowBox,
    pub(super) stack: gtk4::Stack,
    pub(super) empty_title: gtk4::Label,
    pub(super) empty_copy: gtk4::Label,
    pub(super) active_vault_id: Rc<RefCell<Option<Uuid>>>,
    pub(super) toast_overlay: adw::ToastOverlay,
    pub(super) filter_runtime: FilterRuntime,
    pub(super) editor_launcher: Rc<RefCell<Option<Rc<dyn Fn(DialogMode)>>>>,
    /// Last loaded rows (metadata only, never secret values), re-rendered on layout change.
    pub(super) last_loaded: RefCell<Option<Rc<LoadedSecrets>>>,
}

/// Result of one list load, kept to rebuild the widgets without touching the database.
pub(super) struct LoadedSecrets {
    vault_state: Option<(Uuid, bool, bool, bool)>,
    rows: Vec<SecretRowView>,
    /// Live usage counters, shared with the cards so a re-render keeps the counts bumped
    /// by quick actions since the load.
    usage_counts: HashMap<Uuid, Rc<Cell<u32>>>,
    no_selection: bool,
    is_global: bool,
}

/// Load the secrets of the active vault (or of every vault when `search_all_vaults`) in a
/// worker thread, then render them.
pub(super) fn refresh_secret_flow<TSecret, TVault>(
    ctx: &Rc<SecretFlowContext<TSecret, TVault>>,
    admin_master_key: Zeroizing<Vec<u8>>,
    search_all_vaults: bool,
) where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let runtime_handle = ctx.runtime_handle.clone();
    let secret_service = Arc::clone(&ctx.secret_service);
    let vault_service = Arc::clone(&ctx.vault_service);
    let admin_user_id = ctx.user_id;
    let active_vault_id = Rc::clone(&ctx.active_vault_id);
    let empty_title = ctx.empty_title.clone();
    let empty_copy = ctx.empty_copy.clone();
    let stack = ctx.stack.clone();

    empty_title.set_text(heelonvault_core::tr!("main-secrets-loading-title").as_str());
    empty_copy.set_text(heelonvault_core::tr!("main-secrets-loading-description").as_str());
    stack.set_visible_child_name("empty");

    let runtime_for_loader = runtime_handle.clone();
    let secret_for_loader = Arc::clone(&secret_service);
    let vault_for_loader = Arc::clone(&vault_service);
    let admin_master_for_loader = admin_master_key;
    let selected_vault_id = *active_vault_id.borrow();

    let (sender, receiver) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result: Result<
            (
                Option<(Uuid, bool, bool, bool)>,
                Vec<SecretRowView>,
                bool,
                bool,
            ),
            heelonvault_core::errors::AppError,
        > = runtime_for_loader.block_on(async move {
            if search_all_vaults {
                // ── Global cross-vault search ─────────────────────────────────
                let vaults = vault_for_loader.list_user_vaults(admin_user_id).await?;
                let mut all_rows: Vec<(SecretRowView, Option<SecretFingerprint>)> = Vec::new();
                for vault in vaults {
                    let access = match vault_for_loader
                        .get_vault_access_for_user(admin_user_id, vault.id)
                        .await?
                    {
                        Some(a) => a,
                        None => continue,
                    };
                    let vault_key = match vault_for_loader
                        .open_vault_for_user(
                            admin_user_id,
                            vault.id,
                            SecretBox::new(Box::new(admin_master_for_loader.to_vec())),
                        )
                        .await
                    {
                        Ok(k) => k,
                        Err(_) => continue,
                    };
                    let items = match secret_for_loader.list_by_vault(vault.id).await {
                        Ok(i) => i,
                        Err(_) => continue,
                    };
                    let is_shared = vault.owner_user_id != admin_user_id;
                    let can_write = access.role.can_write();
                    let can_admin = access.role.can_admin();
                    for item in items {
                        let row = build_secret_row(
                            item,
                            &secret_for_loader,
                            &vault_key,
                            vault.name.clone(),
                            (is_shared, can_write, can_admin),
                        )
                        .await;
                        if let Some(r) = row {
                            all_rows.push(r);
                        }
                    }
                }
                Ok((None, finalize_rows(all_rows), false, true))
            } else {
                // ── Single-vault mode (normal) ────────────────────────────────
                let vaults = vault_for_loader.list_user_vaults(admin_user_id).await?;
                let resolved_selected_id =
                    selected_vault_id.or_else(|| vaults.first().map(|vault| vault.id));
                let Some(selected_id) = resolved_selected_id else {
                    return Ok((None, Vec::new(), true, false));
                };

                let selected_vault = match vaults.into_iter().find(|vault| vault.id == selected_id)
                {
                    Some(value) => value,
                    None => return Ok((None, Vec::new(), false, false)),
                };
                let access = vault_for_loader
                    .get_vault_access_for_user(admin_user_id, selected_vault.id)
                    .await?
                    .ok_or({
                        heelonvault_core::errors::AppError::Authorization(
                            heelonvault_core::errors::AccessDeniedReason::VaultAccessDenied,
                        )
                    })?;
                let is_shared = selected_vault.owner_user_id != admin_user_id;
                let can_write = access.role.can_write();
                let can_admin = access.role.can_admin();
                let vault_state = Some((selected_vault.id, is_shared, can_write, can_admin));

                let vault_key = vault_for_loader
                    .open_vault_for_user(
                        admin_user_id,
                        selected_vault.id,
                        SecretBox::new(Box::new(admin_master_for_loader.to_vec())),
                    )
                    .await?;

                let items = secret_for_loader.list_by_vault(selected_vault.id).await?;
                let mut rows = Vec::with_capacity(items.len());
                for item in items {
                    if let Some(r) = build_secret_row(
                        item,
                        &secret_for_loader,
                        &vault_key,
                        String::new(),
                        (is_shared, can_write, can_admin),
                    )
                    .await
                    {
                        rows.push(r);
                    }
                }
                Ok((vault_state, finalize_rows(rows), false, false))
            }
        });
        let _ = sender.send(result);
    });

    let ctx_for_receiver = Rc::clone(ctx);
    glib::MainContext::default().spawn_local(async move {
        match receiver.await {
            Ok(Ok((vault_state, rows, no_selection, is_global))) => {
                let usage_counts = rows
                    .iter()
                    .map(|row| (row.secret_id, Rc::new(Cell::new(row.usage_count))))
                    .collect();
                let loaded = Rc::new(LoadedSecrets {
                    vault_state,
                    rows,
                    usage_counts,
                    no_selection,
                    is_global,
                });
                *ctx_for_receiver.last_loaded.borrow_mut() = Some(Rc::clone(&loaded));
                render_secret_rows(&ctx_for_receiver, &loaded);
            }
            Ok(Err(_)) | Err(_) => {
                *ctx_for_receiver.last_loaded.borrow_mut() = None;
                empty_title.set_text(heelonvault_core::tr!("main-list-unavailable-title").as_str());
                empty_copy
                    .set_text(heelonvault_core::tr!("main-list-unavailable-description").as_str());
                stack.set_visible_child_name("empty");
            }
        }
    });
}

/// Rebuild the secret widgets from the last load, without database access or decryption
/// (layout switch). Returns `false` when nothing has been loaded yet.
pub(super) fn rerender_secret_flow<TSecret, TVault>(
    ctx: &Rc<SecretFlowContext<TSecret, TVault>>,
) -> bool
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let loaded = ctx.last_loaded.borrow().clone();
    match loaded {
        Some(loaded) => {
            render_secret_rows(ctx, &loaded);
            true
        }
        None => false,
    }
}

/// Decrypts one password on demand for the clipboard; never keeps it.
struct PasswordCopier<TSecret, TVault> {
    secret_service: Arc<TSecret>,
    vault_service: Arc<TVault>,
    runtime_handle: Handle,
    session_master_key: Rc<RefCell<Vec<u8>>>,
    toast_overlay: adw::ToastOverlay,
    user_id: Uuid,
}

impl<TSecret, TVault> PasswordCopier<TSecret, TVault>
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    /// Re-open the vault (re-checking access, so a revoked share cannot be copied), decrypt
    /// the secret, copy it to the clipboard and wipe it. `on_done(copied)` runs on the GTK
    /// thread once the attempt is over.
    fn copy(&self, secret_id: Uuid, vault_id: Uuid, on_done: impl FnOnce(bool) + 'static) {
        let Some(master_key) =
            super::MainWindow::snapshot_session_master_key(&self.session_master_key)
        else {
            self.toast_overlay.add_toast(adw::Toast::new(
                heelonvault_core::tr!("main-copy-session-locked").as_str(),
            ));
            on_done(false);
            return;
        };
        let master_key = SecretBox::new(Box::new(master_key));
        // Shown by the header indicator until the value is in the clipboard (or the copy fails).
        let decrypting = sensitive_clipboard::begin_decrypting();

        let secret_service = Arc::clone(&self.secret_service);
        let vault_service = Arc::clone(&self.vault_service);
        let runtime_handle = self.runtime_handle.clone();
        let user_id = self.user_id;
        let (sender, receiver) =
            tokio::sync::oneshot::channel::<Result<SecretBox<Vec<u8>>, AppError>>();
        std::thread::spawn(move || {
            let result = runtime_handle.block_on(async move {
                let vault_key = vault_service
                    .open_vault_for_user(user_id, vault_id, master_key)
                    .await?;
                let secret = secret_service.get_secret(secret_id, vault_key).await?;
                Ok(secret.secret_value)
            });
            let _ = sender.send(result);
        });

        let toast_overlay = self.toast_overlay.clone();
        glib::MainContext::default().spawn_local(async move {
            let copied = match receiver.await {
                Ok(Ok(secret_value)) => std::str::from_utf8(secret_value.expose_secret())
                    .is_ok_and(|text| {
                        sensitive_clipboard::copy_sensitive(
                            text,
                            sensitive_clipboard::SensitiveKind::Password,
                            sensitive_clipboard::SECRET_CLEAR_DELAY,
                        )
                    }),
                Ok(Err(_)) | Err(_) => false,
            };
            drop(decrypting);
            let message = if copied {
                messages::toast_password_copied(sensitive_clipboard::SECRET_CLEAR_DELAY)
            } else {
                heelonvault_core::tr!("main-copy-failed")
            };
            toast_overlay.add_toast(adw::Toast::new(message.as_str()));
            on_done(copied);
        });
    }
}

impl SecretRowView {
    /// Usage count including quick actions performed since the load.
    fn usage_count_live(&self, usage_counts: &HashMap<Uuid, Rc<Cell<u32>>>) -> u32 {
        usage_counts
            .get(&self.secret_id)
            .map_or(self.usage_count, |count| count.get())
    }
}

/// Build the secret widgets for `loaded` in the current layout.
fn render_secret_rows<TSecret, TVault>(
    ctx: &Rc<SecretFlowContext<TSecret, TVault>>,
    loaded: &LoadedSecrets,
) where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let LoadedSecrets {
        vault_state,
        rows: items,
        usage_counts,
        no_selection,
        is_global,
    } = loaded;
    let usage_of = |row: &SecretRowView| -> Rc<Cell<u32>> {
        usage_counts
            .get(&row.secret_id)
            .cloned()
            .unwrap_or_else(|| Rc::new(Cell::new(row.usage_count)))
    };
    let (vault_state, no_selection, is_global) = (*vault_state, *no_selection, *is_global);
    let active_vault_for_receiver = Rc::clone(&ctx.active_vault_id);
    let secret_flow = ctx.secret_flow.clone();
    let stack = ctx.stack.clone();
    let empty_title = ctx.empty_title.clone();
    let empty_copy = ctx.empty_copy.clone();
    let filter_runtime = ctx.filter_runtime.clone();
    let secret_service = Arc::clone(&ctx.secret_service);
    let runtime_handle = ctx.runtime_handle.clone();
    let toast_overlay = ctx.toast_overlay.clone();
    let parent_window = ctx.parent_window.clone();
    let editor_launcher = Rc::clone(&ctx.editor_launcher);
    let admin_user_id = ctx.user_id;
    let password_copier = Rc::new(PasswordCopier {
        secret_service: Arc::clone(&ctx.secret_service),
        vault_service: Arc::clone(&ctx.vault_service),
        runtime_handle: ctx.runtime_handle.clone(),
        session_master_key: Rc::clone(&ctx.session_master_key),
        toast_overlay: ctx.toast_overlay.clone(),
        user_id: ctx.user_id,
    });

    if let Some((vault_id, _, _, _)) = vault_state {
        *active_vault_for_receiver.borrow_mut() = Some(vault_id);
    }

    if no_selection {
        empty_title.set_text("Aucun coffre sélectionné");
        empty_copy
            .set_text("Sélectionnez un coffre dans la barre latérale pour afficher ses secrets.");
        stack.set_visible_child_name("empty");
        return;
    }

    // In global search mode vault_state is intentionally None (cross-vault)
    // — only treat a None vault_state as an error in single-vault mode.
    if vault_state.is_none() && !is_global {
        *active_vault_for_receiver.borrow_mut() = None;
        empty_title.set_text("Coffre non disponible");
        empty_copy
            .set_text("Le coffre sélectionné n'est plus accessible. Sélectionnez-en un autre.");
        stack.set_visible_child_name("empty");
        return;
    }

    filter_runtime.meta_by_widget.borrow_mut().clear();
    filter_runtime.actions_by_widget.borrow_mut().clear();
    filter_runtime.audit_all_count_label.set_text("0");
    filter_runtime.audit_weak_count_label.set_text("0");
    filter_runtime.audit_duplicate_count_label.set_text("0");
    filter_runtime.total_count_label.set_text("0");
    filter_runtime.non_compliant_count_label.set_text("0");
    filter_runtime.filtered_status_page.set_visible(false);

    while let Some(child) = secret_flow.first_child() {
        secret_flow.remove(&child);
    }

    if items.is_empty() {
        empty_title.set_text(heelonvault_core::tr!("main-empty-title").as_str());
        empty_copy.set_text(heelonvault_core::tr!("main-empty-description").as_str());
        stack.set_visible_child_name("empty");
        return;
    }

    // Phase 3: sort in data-preparation stage (not during widget rendering)
    // so frequent secrets remain first even for large lists.
    let mut items: Vec<&SecretRowView> = items.iter().collect();
    items.sort_by(|left, right| {
        right
            .usage_count_live(usage_counts)
            .cmp(&left.usage_count_live(usage_counts))
            .then_with(|| left.title.cmp(&right.title))
    });

    let shared_vault = vault_state
        .map(|(_, is_shared, _, _)| is_shared)
        .unwrap_or(false);
    let can_write = vault_state
        .map(|(_, _, can_write, _)| can_write)
        .unwrap_or(false);
    let can_admin = vault_state
        .map(|(_, _, _, can_admin)| can_admin)
        .unwrap_or(false);
    for (original_rank, item) in items.into_iter().enumerate() {
        let is_duplicate = item.is_duplicate;

        // In multi-vault search mode the vault_state is None, so use
        // the per-item vault_access tuple instead.
        let (item_shared, item_can_write, item_can_admin) = if vault_state.is_some() {
            (shared_vault, can_write, can_admin)
        } else {
            item.vault_access
        };

        let card_data = SecretRowData {
            secret_id: item.secret_id,
            icon_name: item.icon_name.clone(),
            type_label: item.type_label.clone(),
            title: item.title.clone(),
            created_at: item.created_at.clone(),
            login: item.login.clone(),
            url: item.url.clone(),
            has_secret: item.has_secret,
            color_class: item.color_class.clone(),
            is_weak: item.is_weak,
            is_health_access: item.is_health_access,
            usage_count: usage_of(item).get(),
            is_duplicate,
            is_incomplete: item.login.trim().is_empty() || item.url.trim().is_empty(),
            is_shared_vault: item_shared,
            can_edit: !item_shared || item_can_write,
            can_delete: !item_shared || item_can_admin,
            vault_name: item.vault_name.clone(),
        };

        let card = Rc::new(SecretCard::new(card_data, filter_runtime.view_mode.get()));
        let copy_button = card.get_copy_button();
        let copy_login_button = card.get_copy_login_button();
        let open_url_button = card.get_open_url_button();
        let usage_count = usage_of(item);
        let kind = item.kind;

        // ── Copy password (🔑) ─────────────────────────────────────────────────────
        // The value is not held by the UI: each copy decrypts it again (see PasswordCopier).
        // copy_button is already desensitised in SecretCard::new when there is no value.
        if item.has_secret {
            let copy_title_for_audit = item.title.clone();
            let card_for_copy = Rc::clone(&card);
            let service_for_copy = Arc::clone(&secret_service);
            let runtime_for_copy = runtime_handle.clone();
            let usage_for_copy = Rc::clone(&usage_count);
            let secret_id_for_copy = item.secret_id;
            let vault_id_for_copy = item.vault_id;
            let copier_for_copy = Rc::clone(&password_copier);
            let copy_in_flight = Rc::new(Cell::new(false));
            copy_button.connect_clicked(move |button| {
                // Ctrl+C emits `clicked` even on an insensitive button: guard explicitly.
                if copy_in_flight.replace(true) {
                    return;
                }
                button.set_sensitive(false);

                let button_for_done = button.clone();
                let in_flight_for_done = Rc::clone(&copy_in_flight);
                let card_for_done = Rc::clone(&card_for_copy);
                let usage_for_done = Rc::clone(&usage_for_copy);
                let service_for_done = Arc::clone(&service_for_copy);
                let runtime_for_done = runtime_for_copy.clone();
                let title_for_done = copy_title_for_audit.clone();
                copier_for_copy.copy(secret_id_for_copy, vault_id_for_copy, move |copied| {
                    in_flight_for_done.set(false);
                    button_for_done.set_sensitive(true);
                    if !copied {
                        return;
                    }

                    let new_value = usage_for_done.get().saturating_add(1);
                    usage_for_done.set(new_value);
                    card_for_done.update_usage_count(new_value);

                    let service_for_task = Arc::clone(&service_for_done);
                    let runtime_for_task = runtime_for_done.clone();
                    std::thread::spawn(move || {
                        let _ = runtime_for_task.block_on(async move {
                            service_for_task
                                .increment_usage_count(secret_id_for_copy)
                                .await
                        });
                    });

                    // ── CNIL: log field copy ──────────────────────
                    std::thread::spawn(move || {
                        let _ = runtime_for_done.block_on(async move {
                            service_for_done
                                .record_field_copy(
                                    secret_id_for_copy,
                                    Some(admin_user_id),
                                    Some(title_for_done.as_str()),
                                    "password",
                                )
                                .await
                        });
                    });
                });
            });
        }

        // ── Copy login (👤) ────────────────────────────────────────────────────────
        // Button is only present when login is non-empty (see SecretCard::new).
        if let Some(copy_login_btn) = copy_login_button.clone() {
            let login_value = item.login.clone();
            let login_title_for_audit = item.title.clone();
            let card_for_login = Rc::clone(&card);
            let service_for_login = Arc::clone(&secret_service);
            let runtime_for_login = runtime_handle.clone();
            let usage_for_login = Rc::clone(&usage_count);
            let secret_id_for_login = item.secret_id;
            let toast_overlay_for_login = toast_overlay.clone();
            copy_login_btn.connect_clicked(move |_| {
                sensitive_clipboard::copy_sensitive(
                    &login_value,
                    sensitive_clipboard::SensitiveKind::Login,
                    sensitive_clipboard::SECRET_CLEAR_DELAY,
                );
                toast_overlay_for_login.add_toast(adw::Toast::new(
                    messages::toast_login_copied(sensitive_clipboard::SECRET_CLEAR_DELAY).as_str(),
                ));

                let new_value = usage_for_login.get().saturating_add(1);
                usage_for_login.set(new_value);
                card_for_login.update_usage_count(new_value);

                let service_for_task = Arc::clone(&service_for_login);
                let runtime_for_task = runtime_for_login.clone();
                std::thread::spawn(move || {
                    let _ = runtime_for_task.block_on(async move {
                        service_for_task
                            .increment_usage_count(secret_id_for_login)
                            .await
                    });
                });

                // CNIL: log login copy.
                let service_for_audit = Arc::clone(&service_for_login);
                let runtime_for_audit = runtime_for_login.clone();
                let title_for_audit = login_title_for_audit.clone();
                std::thread::spawn(move || {
                    let _ = runtime_for_audit.block_on(async move {
                        service_for_audit
                            .record_field_copy(
                                secret_id_for_login,
                                Some(admin_user_id),
                                Some(title_for_audit.as_str()),
                                "login",
                            )
                            .await
                    });
                });
            });
        }

        // ── Open URL (🌐) ──────────────────────────────────────────────────────────
        // Button is only present when url is non-empty (see SecretCard::new).
        if let Some(open_url_btn) = open_url_button.clone() {
            let url_value = item.url.clone();
            let login_value_for_url = item.login.clone();
            let url_title_for_audit = item.title.clone();
            let card_for_url = Rc::clone(&card);
            let service_for_url = Arc::clone(&secret_service);
            let runtime_for_url = runtime_handle.clone();
            let usage_for_url = Rc::clone(&usage_count);
            let secret_id_for_url = item.secret_id;
            let toast_overlay_for_url = toast_overlay.clone();
            let parent_window_for_url = parent_window.clone();
            open_url_btn.connect_clicked(move |_| {
                let copied_login = !login_value_for_url.trim().is_empty()
                    && sensitive_clipboard::copy_sensitive(
                        &login_value_for_url,
                        sensitive_clipboard::SensitiveKind::Login,
                        sensitive_clipboard::SECRET_CLEAR_DELAY,
                    );

                gtk4::show_uri(
                    Some(&parent_window_for_url),
                    &url_value,
                    gtk4::gdk::CURRENT_TIME,
                );
                let toast_message = if copied_login {
                    messages::toast_url_opened_login_copied(sensitive_clipboard::SECRET_CLEAR_DELAY)
                } else {
                    messages::toast_url_opened()
                };
                toast_overlay_for_url.add_toast(adw::Toast::new(toast_message.as_str()));

                let new_value = usage_for_url.get().saturating_add(1);
                usage_for_url.set(new_value);
                card_for_url.update_usage_count(new_value);

                let service_for_task = Arc::clone(&service_for_url);
                let runtime_for_task = runtime_for_url.clone();
                std::thread::spawn(move || {
                    let _ = runtime_for_task.block_on(async move {
                        service_for_task
                            .increment_usage_count(secret_id_for_url)
                            .await
                    });
                });

                // CNIL: log URL open.
                let service_for_audit = Arc::clone(&service_for_url);
                let runtime_for_audit = runtime_for_url.clone();
                let title_for_audit = url_title_for_audit.clone();
                std::thread::spawn(move || {
                    runtime_for_audit.block_on(async move {
                        let _ = service_for_audit
                            .record_field_copy(
                                secret_id_for_url,
                                Some(admin_user_id),
                                Some(title_for_audit.as_str()),
                                "url_open",
                            )
                            .await;

                        if copied_login {
                            let _ = service_for_audit
                                .record_field_copy(
                                    secret_id_for_url,
                                    Some(admin_user_id),
                                    Some(title_for_audit.as_str()),
                                    "login",
                                )
                                .await;
                        }
                    });
                });
            });
        }

        let card_widget = card.get_widget();
        let card_widget_for_hover = card_widget.clone().upcast::<gtk4::Widget>();
        let flow_for_hover = secret_flow.clone();
        let hover_controller = gtk4::EventControllerMotion::new();
        hover_controller.connect_enter(move |_controller, _x, _y| {
            if let Some(parent) = card_widget_for_hover.parent()
                && let Ok(flow_child) = parent.downcast::<gtk4::FlowBoxChild>()
            {
                flow_for_hover.select_child(&flow_child);
                flow_child.grab_focus();
            }
        });
        card_widget.add_controller(hover_controller);

        // Keep keyboard shortcuts aligned with the last card targeted by mouse.
        let card_widget_for_select = card_widget.clone().upcast::<gtk4::Widget>();
        let flow_for_select = secret_flow.clone();
        let select_click = gtk4::GestureClick::new();
        select_click.set_button(0);
        select_click.connect_pressed(move |_, _, _, _| {
            if let Some(parent) = card_widget_for_select.parent()
                && let Ok(flow_child) = parent.downcast::<gtk4::FlowBoxChild>()
            {
                flow_for_select.select_child(&flow_child);
                flow_child.grab_focus();
            }
        });
        card_widget.add_controller(select_click);

        // Open editor when clicking the card (outside quick-action buttons).
        if !item_shared || item_can_write {
            let editor_launcher_for_card = editor_launcher.clone();
            let secret_id_for_card = item.secret_id;
            let card_widget_for_pick = card_widget.clone().upcast::<gtk4::Widget>();
            let card_click = gtk4::GestureClick::new();
            card_click.set_button(0);
            card_click.connect_released(move |_, n_press, x, y| {
                if n_press < 2 {
                    return;
                }

                if let Some(picked) = card_widget_for_pick.pick(x, y, gtk4::PickFlags::DEFAULT) {
                    let mut current = Some(picked);
                    while let Some(widget) = current {
                        if widget.has_css_class("secret-card-action-btn") {
                            return;
                        }
                        current = widget.parent();
                    }
                }

                if let Some(open_editor) = editor_launcher_for_card.borrow().as_ref() {
                    open_editor(DialogMode::Edit(secret_id_for_card));
                }
            });
            card_widget.add_controller(card_click);
        }

        let widget_key = format!("secret-card-{}", item.secret_id);
        card_widget.set_widget_name(&widget_key);
        filter_runtime.actions_by_widget.borrow_mut().insert(
            widget_key.clone(),
            SecretQuickActions {
                copy_password: copy_button.clone(),
                copy_login: copy_login_button.clone(),
                open_url: open_url_button.clone(),
            },
        );
        filter_runtime.meta_by_widget.borrow_mut().insert(
            widget_key,
            SecretFilterMeta {
                searchable_text: search_filter::normalize_search_text(
                    [
                        item.title.clone(),
                        item.type_label.clone(),
                        item.login.clone(),
                        item.email.clone(),
                        item.url.clone(),
                        item.notes.clone(),
                        item.category.clone(),
                        item.tags.clone(),
                        item.created_at.clone(),
                        strength_label(item.is_weak),
                        item.vault_name.clone(),
                    ]
                    .join(" ")
                    .as_str(),
                ),
                title_text: search_filter::normalize_search_text(item.title.as_str()),
                login_text: search_filter::normalize_search_text(item.login.as_str()),
                email_text: search_filter::normalize_search_text(item.email.as_str()),
                url_text: search_filter::normalize_search_text(item.url.as_str()),
                notes_text: search_filter::normalize_search_text(item.notes.as_str()),
                category_text: search_filter::normalize_search_text(item.category.as_str()),
                tags_text: search_filter::normalize_search_text(item.tags.as_str()),
                type_text: search_filter::normalize_search_text(
                    [
                        item.type_label.clone(),
                        match kind {
                            SecretKind::Password => "password motdepasse mdp".to_string(),
                            SecretKind::ApiToken => "token api acces".to_string(),
                            SecretKind::SshKey => "ssh cle key".to_string(),
                            SecretKind::SecureDocument => "document fichier".to_string(),
                        },
                    ]
                    .join(" ")
                    .as_str(),
                ),
                vault_name_text: search_filter::normalize_search_text(item.vault_name.as_str()),
                kind,
                original_rank,
                is_weak: item.is_weak,
                is_duplicate,
                is_health: item.is_health_access,
                is_incomplete: item.login.trim().is_empty() || item.url.trim().is_empty(),
                is_never_used: usage_count.get() == 0,
            },
        );
        secret_flow.insert(&card_widget, -1);
    }

    search_filter::apply_filters(&secret_flow, &filter_runtime);
    stack.set_visible_child_name("list");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_password_detection_is_language_independent() {
        assert!(is_weak_password(""));
        assert!(is_weak_password("Short1!"));
        assert!(is_weak_password("onlylowercaseletters"));
        assert!(!is_weak_password("Correct-Horse-42"));
        assert!(!is_weak_password("Éléphant-Rose-2026"));
    }

    #[test]
    fn empty_value_has_no_fingerprint() {
        assert_eq!(secret_fingerprint(b""), None);
        assert!(secret_fingerprint(b"x").is_some());
    }

    #[test]
    fn identical_values_share_a_fingerprint() {
        assert_eq!(
            secret_fingerprint("mot de passe é".as_bytes()),
            secret_fingerprint("mot de passe é".as_bytes())
        );
        assert_ne!(secret_fingerprint(b"alpha"), secret_fingerprint(b"Alpha"));
    }

    #[test]
    fn duplicate_flags_mark_every_occurrence_of_a_reused_value() {
        let fingerprints = vec![
            secret_fingerprint(b"reused"),
            secret_fingerprint(b"unique"),
            None,
            secret_fingerprint(b"reused"),
            None,
        ];
        assert_eq!(
            duplicate_flags(&fingerprints),
            vec![true, false, false, true, false]
        );
    }

    #[test]
    fn duplicate_flags_on_empty_batch() {
        assert!(duplicate_flags(&[]).is_empty());
    }
}

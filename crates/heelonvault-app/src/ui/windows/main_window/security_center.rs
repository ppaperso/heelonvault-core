use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

use gtk4::{Align, Orientation, glib, prelude::*};
use heelonvault_core::i18n::{I18nArg, tr, tr_args};
use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::security_center::{
    self, Finding, ScanControl, SecurityEntry, SecurityReport,
};
use heelonvault_core::services::vault_service::VaultService;
use libadwaita as adw;
use secrecy::SecretBox;
use tokio::runtime::Handle;
use uuid::Uuid;

use crate::ui::dialogs::add_edit_dialog::DialogMode;
use crate::ui::widgets::url_security::url_status_key;

pub(super) struct SecurityCenter {
    pub refresh_i18n: Rc<dyn Fn()>,
    pub invalidate: Rc<dyn Fn()>,
    pub clear: Rc<dyn Fn()>,
    #[cfg(test)]
    view: Rc<View>,
}

struct View {
    root: gtk4::Box,
    title: gtk4::Label,
    subtitle: gtk4::Label,
    summary: gtk4::Label,
    coverage: gtk4::Label,
    privacy: gtk4::Label,
    refresh: gtk4::Button,
    cancel: gtk4::Button,
    spinner: gtk4::Spinner,
    cards: Vec<gtk4::Button>,
    vaults: gtk4::DropDown,
    results: gtk4::Box,
    more: gtk4::Button,
    limit: Cell<usize>,
    report: RefCell<Option<SecurityReport>>,
    control: RefCell<Option<Arc<ScanControl>>>,
    generation: Cell<u64>,
    selected: Cell<usize>,
    selected_vault: Cell<Option<Uuid>>,
    updating_vaults: Cell<bool>,
    expanded: Rc<RefCell<BTreeSet<Uuid>>>,
    scroll: gtk4::Adjustment,
    restore_scroll: Cell<f64>,
    busy: Cell<bool>,
    failed: Cell<bool>,
    completed_at: RefCell<Option<String>>,
    open: Rc<dyn Fn(Uuid, Uuid)>,
}

fn label(key: &str, class: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(&tr(key)));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(class);
    label
}

fn matches_filter(entry: &SecurityEntry, filter: usize) -> bool {
    entry.findings.iter().any(|finding| match filter {
        1 => *finding == Finding::ReusedPassword,
        2 => *finding == Finding::WeakPassword,
        3 => matches!(
            finding,
            Finding::Expired | Finding::ExpiringSoon | Finding::InvalidExpiry
        ),
        4 => matches!(finding, Finding::Url(_)),
        _ => true,
    })
}

fn finding_key(finding: Finding) -> &'static str {
    match finding {
        Finding::WeakPassword => "security-finding-weak",
        Finding::ReusedPassword => "security-finding-reused",
        Finding::Expired => "security-finding-expired",
        Finding::ExpiringSoon => "security-finding-expiring",
        Finding::InvalidExpiry => "security-finding-invalid-expiry",
        Finding::Url(status) => url_status_key(status),
        Finding::InvalidMetadata => "security-finding-metadata",
        _ => "security-finding-unreadable",
    }
}

fn priority(entry: &SecurityEntry) -> u8 {
    entry
        .findings
        .iter()
        .map(|finding| match finding {
            Finding::Unreadable | Finding::InvalidMetadata => 0,
            Finding::Expired => 1,
            Finding::ReusedPassword => 2,
            Finding::WeakPassword => 3,
            Finding::Url(_) => 4,
            _ => 5,
        })
        .min()
        .unwrap_or(6)
}

fn short_finding_key(finding: Finding) -> &'static str {
    match finding {
        Finding::WeakPassword => "security-kind-weak",
        Finding::ReusedPassword => "security-kind-reused",
        Finding::Expired => "security-kind-expired",
        Finding::ExpiringSoon => "security-kind-expiring",
        Finding::Url(_) => "security-kind-url",
        _ => "security-kind-unavailable",
    }
}

impl View {
    fn stop(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(control) = self.control.borrow_mut().take() {
            control.cancel();
        }
        self.busy.set(false);
        self.spinner.stop();
        self.cancel.set_visible(false);
        self.refresh.set_sensitive(true);
    }

    fn render(&self) {
        self.title.set_text(&tr("security-center-title"));
        self.subtitle.set_text(&tr("security-center-subtitle"));
        self.privacy.set_text(&tr("security-center-privacy"));
        self.refresh.set_label(&tr("security-center-refresh"));
        self.cancel.set_label(&tr("security-center-cancel"));
        self.more.set_label(&tr("security-center-more"));
        self.more.set_visible(false);
        self.vaults
            .set_tooltip_text(Some(&tr("security-center-vaults")));
        while let Some(child) = self.results.first_child() {
            self.results.remove(&child);
        }
        let stored = self.report.borrow();
        let keys = [
            "security-card-all",
            "security-card-reused",
            "security-card-weak",
            "security-card-expiry",
            "security-card-urls",
        ];
        for (index, (button, key)) in self.cards.iter().zip(keys).enumerate() {
            let count = stored.as_ref().map_or(0, |report| {
                report
                    .entries
                    .iter()
                    .filter(|entry| matches_filter(entry, index))
                    .count()
            });
            button.set_label(&format!("{count}\n{}", tr(key)));
            if index == self.selected.get() {
                button.add_css_class("security-metric-selected");
            } else {
                button.remove_css_class("security-metric-selected");
            }
        }
        if self.busy.get() {
            let count = self
                .control
                .borrow()
                .as_ref()
                .map_or(0, |c| c.processed.load(Ordering::Acquire));
            self.summary.set_text(&tr_args(
                "security-center-progress",
                &[("count", I18nArg::Str(&count.to_string()))],
            ));
            self.coverage.set_text(&tr("security-center-local"));
            return;
        }
        if self.failed.get() {
            self.summary.set_text(&tr("security-center-error"));
            self.coverage.set_text(&tr("security-center-error-detail"));
            return;
        }
        let Some(report) = stored.as_ref() else {
            self.summary.set_text(&tr("security-center-ready"));
            self.coverage.set_text(&tr("security-center-scope"));
            return;
        };
        self.summary.set_text(&tr_args(
            "security-center-summary",
            &[("count", I18nArg::Str(&report.affected().to_string()))],
        ));
        self.coverage.set_text(&tr_args(
            "security-center-coverage",
            &[
                ("count", I18nArg::Str(&report.analyzed.to_string())),
                ("vaults", I18nArg::Str(&report.vaults_analyzed.to_string())),
                (
                    "unavailable",
                    I18nArg::Str(&report.unavailable_vaults.to_string()),
                ),
                ("unreadable", I18nArg::Str(&report.unreadable.to_string())),
                (
                    "time",
                    I18nArg::Str(self.completed_at.borrow().as_deref().unwrap_or_default()),
                ),
            ],
        ));
        if !report.is_complete() {
            self.results
                .append(&label("security-center-partial", "security-notice"));
        }
        let vault_id = self.selected_vault.get();
        let mut selected: Vec<_> = report
            .entries
            .iter()
            .filter(|entry| {
                matches_filter(entry, self.selected.get())
                    && vault_id.is_none_or(|id| entry.vault_id == id)
            })
            .collect();
        selected.sort_by(|left, right| {
            priority(left)
                .cmp(&priority(right))
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.secret_id.cmp(&right.secret_id))
        });
        let mut rendered_groups = BTreeSet::new();
        let mut count = 0;
        self.more.set_visible(selected.len() > self.limit.get());
        // Bound GTK widget construction even when one password is reused thousands of times.
        let visible = &selected[..selected.len().min(self.limit.get())];
        for entry in visible {
            if let Some(group) = entry.reuse_group
                && matches!(self.selected.get(), 0 | 1)
            {
                if !rendered_groups.insert(group) {
                    continue;
                }
                let group_box = gtk4::Box::new(Orientation::Vertical, 8);
                group_box.add_css_class("security-group");
                group_box.append(&label("security-reuse-group", "heading"));
                for member in visible
                    .iter()
                    .copied()
                    .filter(|candidate| candidate.reuse_group == Some(group))
                {
                    group_box.append(&self.entry_widget(member));
                }
                self.results.append(&group_box);
            } else {
                self.results.append(&self.entry_widget(entry));
            }
            count += 1;
        }
        if count == 0 {
            self.results
                .append(&label("security-center-no-findings", "security-empty"));
        }
    }

    fn entry_widget(&self, entry: &SecurityEntry) -> gtk4::Expander {
        let expander = gtk4::Expander::new(None);
        expander.add_css_class("security-finding");
        let header = gtk4::Box::new(Orientation::Horizontal, 12);
        let icon = gtk4::Image::from_icon_name("dialog-warning-symbolic");
        icon.add_css_class("security-attention-icon");
        header.append(&icon);
        let text = gtk4::Box::new(Orientation::Vertical, 4);
        text.set_hexpand(true);
        let title = gtk4::Label::new(Some(&entry.title));
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.add_css_class("heading");
        text.append(&title);
        let kinds: Vec<_> = entry
            .findings
            .iter()
            .map(|&finding| tr(short_finding_key(finding)))
            .collect();
        let vault = gtk4::Label::new(Some(&format!(
            "{} · {}",
            entry.vault_name,
            kinds.join(" · ")
        )));
        vault.set_xalign(0.0);
        vault.set_wrap(true);
        vault.add_css_class("dim-label");
        text.append(&vault);
        header.append(&text);
        expander.set_label_widget(Some(&header));
        let details = gtk4::Box::new(Orientation::Vertical, 10);
        details.add_css_class("security-finding-detail");
        for &finding in &entry.findings {
            details.append(&label(finding_key(finding), "security-reason"));
        }
        details.append(&label("security-center-correction-help", "dim-label"));
        let edit = gtk4::Button::with_label(&tr("security-center-edit"));
        edit.set_halign(Align::Start);
        edit.add_css_class("suggested-action");
        let open = Rc::clone(&self.open);
        let (id, vault_id) = (entry.secret_id, entry.vault_id);
        edit.connect_clicked(move |_| open(id, vault_id));
        details.append(&edit);
        expander.set_child(Some(&details));
        expander.set_expanded(self.expanded.borrow().contains(&id));
        let expanded = Rc::clone(&self.expanded);
        expander.connect_expanded_notify(move |expander| {
            if expander.is_expanded() {
                expanded.borrow_mut().insert(id);
            } else {
                expanded.borrow_mut().remove(&id);
            }
        });
        expander
    }

    fn set_report(self: &Rc<Self>, report: SecurityReport) {
        let names: BTreeSet<_> = report
            .entries
            .iter()
            .map(|entry| (entry.vault_name.clone(), entry.vault_id))
            .collect();
        let mut labels = vec![tr("security-center-vaults")];
        let selection = names
            .iter()
            .position(|(_, id)| Some(*id) == self.selected_vault.get())
            .and_then(|index| u32::try_from(index + 1).ok())
            .unwrap_or(0);
        if selection == 0 {
            self.selected_vault.set(None);
        }
        labels.extend(names.into_iter().map(|(name, _)| name));
        let strings: Vec<_> = labels.iter().map(String::as_str).collect();
        self.updating_vaults.set(true);
        self.vaults
            .set_model(Some(&gtk4::StringList::new(&strings)));
        self.vaults.set_selected(selection);
        self.updating_vaults.set(false);
        *self.completed_at.borrow_mut() = Some(chrono::Local::now().format("%H:%M").to_string());
        *self.report.borrow_mut() = Some(report);
        self.render();
        let weak = Rc::downgrade(self);
        let generation = self.generation.get();
        glib::timeout_add_local_once(Duration::from_millis(40), move || {
            if let Some(view) = weak.upgrade()
                && view.generation.get() == generation
            {
                view.scroll.set_value(
                    view.restore_scroll
                        .get()
                        .min((view.scroll.upper() - view.scroll.page_size()).max(0.0)),
                );
            }
        });
    }
}

pub(super) struct CenterDeps<TSecret, TVault> {
    pub runtime: Handle,
    pub secrets: Arc<TSecret>,
    pub vaults: Arc<TVault>,
    pub user_id: Uuid,
    pub key: Rc<RefCell<Vec<u8>>>,
    pub stack: gtk4::Stack,
    pub navigation: gtk4::Button,
    pub open_editor: Rc<dyn Fn(DialogMode)>,
    pub toast: adw::ToastOverlay,
}

pub(super) fn build<TSecret, TVault>(deps: CenterDeps<TSecret, TVault>) -> SecurityCenter
where
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let page = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let root = gtk4::Box::new(Orientation::Vertical, 20);
    root.add_css_class("security-center");
    page.set_child(Some(&root));
    deps.stack.add_titled(
        &page,
        Some("security_center_view"),
        &tr("security-center-title"),
    );
    let open = {
        let runtime = deps.runtime.clone();
        let vaults = Arc::clone(&deps.vaults);
        let key = Rc::clone(&deps.key);
        let editor = Rc::clone(&deps.open_editor);
        let toast = deps.toast.clone();
        let user_id = deps.user_id;
        Rc::new(move |id, vault_id| {
            let Some(snapshot) = super::MainWindow::snapshot_session_master_key(&key) else {
                return;
            };
            let vaults = Arc::clone(&vaults);
            let key = Rc::clone(&key);
            let editor = Rc::clone(&editor);
            let toast = toast.clone();
            let task = runtime.spawn(async move {
                vaults
                    .open_vault_for_user(user_id, vault_id, SecretBox::new(Box::new(snapshot)))
                    .await
            });
            glib::MainContext::default().spawn_local(async move {
                if key.borrow().is_empty() {
                    task.abort();
                    return;
                }
                match task.await {
                    Ok(Ok(_)) if !key.borrow().is_empty() => editor(DialogMode::Edit(id)),
                    _ => toast.add_toast(adw::Toast::new(&tr("security-center-access-error"))),
                }
            });
        }) as Rc<dyn Fn(Uuid, Uuid)>
    };
    let view = Rc::new(View {
        root,
        title: label("security-center-title", "title-1"),
        subtitle: label("security-center-subtitle", "security-subtitle"),
        summary: label("security-center-ready", "title-2"),
        coverage: label("security-center-scope", "security-coverage"),
        privacy: label("security-center-privacy", "security-privacy"),
        refresh: gtk4::Button::with_label(&tr("security-center-refresh")),
        cancel: gtk4::Button::with_label(&tr("security-center-cancel")),
        spinner: gtk4::Spinner::new(),
        cards: (0..5).map(|_| gtk4::Button::new()).collect(),
        vaults: gtk4::DropDown::from_strings(&[&tr("security-center-vaults")]),
        results: gtk4::Box::new(Orientation::Vertical, 12),
        more: gtk4::Button::with_label(&tr("security-center-more")),
        limit: Cell::new(50),
        report: RefCell::new(None),
        control: RefCell::new(None),
        generation: Cell::new(0),
        selected: Cell::new(0),
        selected_vault: Cell::new(None),
        updating_vaults: Cell::new(false),
        expanded: Rc::new(RefCell::new(BTreeSet::new())),
        scroll: page.vadjustment(),
        restore_scroll: Cell::new(0.0),
        busy: Cell::new(false),
        failed: Cell::new(false),
        completed_at: RefCell::new(None),
        open,
    });
    view.root.append(&view.title);
    view.root.append(&view.subtitle);
    let hero = gtk4::Box::new(Orientation::Horizontal, 20);
    hero.add_css_class("security-hero");
    let shield = gtk4::Image::from_icon_name("security-high-symbolic");
    shield.set_pixel_size(64);
    hero.append(&shield);
    let hero_text = gtk4::Box::new(Orientation::Vertical, 10);
    hero_text.set_hexpand(true);
    hero_text.append(&view.summary);
    hero_text.append(&view.coverage);
    let actions = gtk4::Box::new(Orientation::Horizontal, 8);
    view.refresh.add_css_class("suggested-action");
    actions.append(&view.refresh);
    actions.append(&view.cancel);
    actions.append(&view.spinner);
    hero_text.append(&actions);
    hero.append(&hero_text);
    view.root.append(&hero);
    let cards = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(1)
        .max_children_per_line(5)
        .column_spacing(10)
        .row_spacing(10)
        .build();
    for (index, button) in view.cards.iter().enumerate() {
        button.add_css_class("security-metric");
        cards.insert(button, -1);
        let weak = Rc::downgrade(&view);
        button.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.selected.set(index);
                view.limit.set(50);
                view.render();
            }
        });
    }
    view.root.append(&cards);
    view.root.append(&view.vaults);
    view.root.append(&view.results);
    view.root.append(&view.more);
    view.root.append(&view.privacy);
    view.cancel.set_visible(false);
    let weak = Rc::downgrade(&view);
    view.vaults.connect_selected_notify(move |_| {
        if let Some(view) = weak.upgrade() {
            if view.updating_vaults.get() {
                return;
            }
            let names: BTreeSet<_> = view
                .report
                .borrow()
                .as_ref()
                .map(|report| {
                    report
                        .entries
                        .iter()
                        .map(|entry| (entry.vault_name.clone(), entry.vault_id))
                        .collect()
                })
                .unwrap_or_default();
            let selected = view
                .vaults
                .selected()
                .checked_sub(1)
                .and_then(|index| names.iter().nth(index as usize).map(|(_, id)| *id));
            view.selected_vault.set(selected);
            view.limit.set(50);
            view.render();
        }
    });
    let weak = Rc::downgrade(&view);
    view.more.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            view.limit.set(view.limit.get().saturating_add(50));
            view.render();
        }
    });
    let scan: Rc<dyn Fn()> = {
        let view = Rc::downgrade(&view);
        let runtime = deps.runtime.clone();
        let secrets = Arc::clone(&deps.secrets);
        let vaults = Arc::clone(&deps.vaults);
        let key = Rc::clone(&deps.key);
        Rc::new(move || {
            let Some(view) = view.upgrade() else { return };
            view.restore_scroll.set(view.scroll.value());
            view.stop();
            let Some(snapshot) = super::MainWindow::snapshot_session_master_key(&key) else {
                return;
            };
            *view.report.borrow_mut() = None;
            view.failed.set(false);
            view.busy.set(true);
            view.refresh.set_sensitive(false);
            view.cancel.set_visible(true);
            view.spinner.start();
            let control = Arc::new(ScanControl::default());
            *view.control.borrow_mut() = Some(Arc::clone(&control));
            let generation = view.generation.get();
            let secrets = Arc::clone(&secrets);
            let vaults = Arc::clone(&vaults);
            let worker_runtime = runtime.clone();
            let task = runtime.spawn_blocking(move || {
                worker_runtime.block_on(security_center::analyze(
                    secrets.as_ref(),
                    vaults.as_ref(),
                    deps.user_id,
                    SecretBox::new(Box::new(snapshot)),
                    time::OffsetDateTime::now_utc(),
                    &control,
                ))
            });
            let weak = Rc::downgrade(&view);
            let key_for_progress = Rc::clone(&key);
            glib::timeout_add_local(Duration::from_millis(150), move || {
                let Some(view) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if key_for_progress.borrow().is_empty() {
                    view.stop();
                    return glib::ControlFlow::Break;
                }
                if !view.busy.get() || view.generation.get() != generation {
                    return glib::ControlFlow::Break;
                }
                view.render();
                glib::ControlFlow::Continue
            });
            let weak = Rc::downgrade(&view);
            let key = Rc::clone(&key);
            glib::MainContext::default().spawn_local(async move {
                let result = task.await;
                let Some(view) = weak.upgrade() else { return };
                if generation != view.generation.get() || key.borrow().is_empty() {
                    return;
                }
                view.stop();
                match result {
                    Ok(Ok(report)) => view.set_report(report),
                    _ => {
                        view.failed.set(true);
                        view.render();
                    }
                }
            });
            view.render();
        })
    };
    let start = Rc::clone(&scan);
    view.refresh.connect_clicked(move |_| start());
    let weak = Rc::downgrade(&view);
    view.cancel.connect_clicked(move |_| {
        if let Some(view) = weak.upgrade() {
            view.stop();
            view.render();
        }
    });
    let stack = deps.stack.clone();
    let start = Rc::clone(&scan);
    let weak = Rc::downgrade(&view);
    deps.navigation.connect_clicked(move |_| {
        stack.set_visible_child_name("security_center_view");
        if let Some(view) = weak.upgrade()
            && !view.busy.get()
        {
            start();
        }
    });
    let weak = Rc::downgrade(&view);
    page.connect_unmap(move |_| {
        if let Some(view) = weak.upgrade() {
            view.stop();
        }
    });
    view.render();
    let refresh_i18n = {
        let view = Rc::clone(&view);
        let stack = deps.stack.clone();
        let navigation = deps.navigation.clone();
        let page = page.clone();
        Rc::new(move || {
            navigation.set_label(&tr("security-center-title"));
            stack.page(&page).set_title(&tr("security-center-title"));
            view.render();
            if let Some(model) = view.vaults.model().and_downcast::<gtk4::StringList>() {
                model.splice(0, 1, &[&tr("security-center-vaults")]);
            }
        }) as Rc<dyn Fn()>
    };
    let invalidate = {
        let view = Rc::downgrade(&view);
        let stack = deps.stack.clone();
        Rc::new(move || {
            let Some(view) = view.upgrade() else { return };
            if stack.visible_child_name().as_deref() == Some("security_center_view") {
                scan();
            } else {
                view.stop();
                *view.report.borrow_mut() = None;
            }
        }) as Rc<dyn Fn()>
    };
    let view_for_clear = Rc::clone(&view);
    let clear = Rc::new(move || {
        let view = &view_for_clear;
        view.stop();
        *view.report.borrow_mut() = None;
        view.selected_vault.set(None);
        view.expanded.borrow_mut().clear();
        view.vaults.set_model(Some(&gtk4::StringList::new(&[&tr(
            "security-center-vaults",
        )])));
        view.render();
    });
    SecurityCenter {
        refresh_i18n,
        invalidate,
        clear,
        #[cfg(test)]
        view,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heelonvault_core::repositories::{
        secret_repository::SqlxSecretRepository, team_repository::SqlxTeamRepository,
        user_repository::SqlxUserRepository, vault_repository::SqlxVaultRepository,
    };
    use heelonvault_core::services::{
        audit_log_service::NoOpAuditLogService, crypto_service::CryptoServiceImpl,
        secret_service::SecretServiceImpl, vault_service::VaultServiceImpl,
    };

    #[test]
    fn filters_and_priorities_are_explicit() {
        let mut entry = SecurityEntry {
            secret_id: Uuid::nil(),
            vault_id: Uuid::nil(),
            title: String::new(),
            vault_name: String::new(),
            findings: vec![Finding::WeakPassword, Finding::ReusedPassword],
            reuse_group: Some(0),
        };
        assert!(matches_filter(&entry, 0));
        assert!(matches_filter(&entry, 1));
        assert!(matches_filter(&entry, 2));
        assert!(!matches_filter(&entry, 3));
        assert!(!matches_filter(&entry, 4));
        assert_eq!(priority(&entry), 2);
        entry.findings.push(Finding::Unreadable);
        assert_eq!(priority(&entry), 0);
    }

    #[test]
    #[ignore = "requires a graphical display; run explicitly for native GTK validation"]
    fn native_security_center_smoke() -> Result<(), Box<dyn std::error::Error>> {
        adw::init()?;
        gio::resources_register_include!("heelonvault.gresource")?;
        let display = gtk4::gdk::Display::default().ok_or("no graphical display")?;
        let css = gtk4::CssProvider::new();
        css.connect_parsing_error(|_, section, error| {
            eprintln!("CSS error at {section:?}: {error}");
        });
        css.load_from_resource("/com/heelonvault/rust/style.css");
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let runtime = tokio::runtime::Runtime::new()?;
        let _entered = runtime.enter();
        let password_entry = gtk4::PasswordEntry::new();
        let strength =
            crate::ui::widgets::password_strength_bar::PasswordStrengthBar::for_stored_passwords();
        strength.connect_to_password_entry(&password_entry);
        for value in ["Password123!", " eW8!rK2#qL7$vN4@zT9 ", "àÉ8!xyz", ""] {
            password_entry.set_text(value);
            assert_eq!(
                strength.last_score(),
                i32::from(security_center::assess_password(value, "").score)
            );
        }
        let pool = sqlx::sqlite::SqlitePoolOptions::new().connect_lazy("sqlite::memory:")?;
        let panel = super::super::center::build_center_panel(
            crate::ui::view_preferences::SecretViewMode::Grid,
        );
        let sidebar = super::super::sidebar::build_sidebar_panel();
        let toast = adw::ToastOverlay::new();
        let secret_service = Arc::new(SecretServiceImpl::new(
            SqlxSecretRepository::new(pool.clone()),
            CryptoServiceImpl::default(),
            Arc::new(NoOpAuditLogService),
        ));
        let vault_service = Arc::new(VaultServiceImpl::new(
            SqlxVaultRepository::new(pool.clone()),
            SqlxVaultRepository::new(pool.clone()),
            SqlxUserRepository::new(pool.clone()),
            SqlxTeamRepository::new(pool),
            Arc::new(NoOpAuditLogService),
            CryptoServiceImpl::default(),
        ));
        let center = build(CenterDeps {
            runtime: runtime.handle().clone(),
            secrets: secret_service,
            vaults: vault_service,
            user_id: Uuid::nil(),
            key: Rc::new(RefCell::new(vec![1; 32])),
            stack: panel.main_stack.clone(),
            navigation: sidebar.security_center_button.clone(),
            open_editor: Rc::new(|_| {}),
            toast: toast.clone(),
        });
        let vault_id = Uuid::new_v4();
        let entries = vec![
            (
                "Messagerie",
                vec![Finding::WeakPassword, Finding::ReusedPassword],
                Some(0),
            ),
            (
                "Boutique",
                vec![Finding::WeakPassword, Finding::ReusedPassword],
                Some(0),
            ),
            ("API production", vec![Finding::Expired], None),
            (
                "Portail client",
                vec![Finding::Url(security_center::UrlStatus::Http)],
                None,
            ),
        ]
        .into_iter()
        .map(|(title, findings, reuse_group)| SecurityEntry {
            secret_id: Uuid::new_v4(),
            vault_id,
            title: title.to_string(),
            vault_name: "Personnel".to_string(),
            findings,
            reuse_group,
        })
        .collect();
        center.view.set_report(SecurityReport {
            entries,
            analyzed: 124,
            vaults_analyzed: 3,
            ..SecurityReport::default()
        });
        assert_eq!(
            center
                .view
                .report
                .borrow()
                .as_ref()
                .map(SecurityReport::affected),
            Some(4)
        );
        center.view.cards[4].emit_clicked();
        assert_eq!(center.view.selected.get(), 4);
        assert!(
            center
                .view
                .results
                .first_child()
                .is_some_and(|c| c.next_sibling().is_none())
        );
        center.view.cards[0].emit_clicked();
        center.view.vaults.set_selected(1);
        assert_eq!(center.view.selected_vault.get(), Some(vault_id));
        let original = center
            .view
            .report
            .borrow()
            .clone()
            .ok_or("missing report")?;
        center.view.set_report(original.clone());
        assert_eq!(center.view.selected_vault.get(), Some(vault_id));
        center.view.vaults.set_selected(0);
        let shell = gtk4::Box::new(Orientation::Horizontal, 16);
        shell.append(&sidebar.frame);
        shell.append(&panel.frame);
        toast.set_child(Some(&shell));
        let window = gtk4::Window::builder()
            .title("HeelonVault — Security center preview")
            .default_width(1180)
            .default_height(850)
            .child(&toast)
            .build();
        panel
            .main_stack
            .set_visible_child_name("security_center_view");
        window.present();
        let context = glib::MainContext::default();
        let drain = || {
            let until = std::time::Instant::now() + Duration::from_millis(300);
            while std::time::Instant::now() < until {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        drain();
        for (width, height, name) in [
            (1180, 850, "security-center-wide.png"),
            (820, 850, "security-center-compact.png"),
        ] {
            window.set_default_size(width, height);
            drain();
            if let Some(dir) = std::env::var_os("HEELONVAULT_UI_SNAPSHOT_DIR") {
                let paintable = gtk4::WidgetPaintable::new(Some(&window));
                let snapshot = gtk4::Snapshot::new();
                paintable.snapshot(
                    &snapshot,
                    f64::from(window.width()),
                    f64::from(window.height()),
                );
                let node = snapshot.to_node().ok_or("empty GTK snapshot")?;
                let surface = window.surface().ok_or("missing surface")?;
                let renderer =
                    gtk4::gsk::Renderer::for_surface(&surface).ok_or("missing renderer")?;
                renderer
                    .render_texture(&node, None)
                    .save_to_png(std::path::PathBuf::from(dir).join(name))?;
                renderer.unrealize();
            }
        }
        let mut large = original;
        large.entries = (0..10_000)
            .map(|_| SecurityEntry {
                secret_id: Uuid::new_v4(),
                vault_id,
                title: "Synthetic repeated item".to_string(),
                vault_name: "Fixture".to_string(),
                findings: vec![Finding::ReusedPassword],
                reuse_group: Some(0),
            })
            .collect();
        large.analyzed = large.entries.len();
        center.view.set_report(large);
        assert!(center.view.more.is_visible());
        let group = center
            .view
            .results
            .first_child()
            .and_downcast::<gtk4::Box>()
            .ok_or("missing reuse group")?;
        let mut children = 0;
        let mut child = group.first_child();
        while let Some(widget) = child {
            children += 1;
            child = widget.next_sibling();
        }
        assert_eq!(
            children, 51,
            "one group heading plus exactly 50 rendered items"
        );
        center.view.more.emit_clicked();
        assert_eq!(center.view.limit.get(), 100);
        (center.clear)();
        assert!(center.view.report.borrow().is_none());
        assert!(!center.view.busy.get());
        window.close();
        drain();
        Ok(())
    }
}

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;
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
    hero: gtk4::Box,
    shield: gtk4::Image,
    nav: super::SecurityNavigation,
    cards: Vec<MetricCard>,
    vaults: gtk4::DropDown,
    results: gtk4::Box,
    more: gtk4::Button,
    limit: Cell<usize>,
    report: RefCell<Option<SecurityReport>>,
    control: RefCell<Option<Arc<ScanControl>>>,
    pending: RefCell<Option<glib::SourceId>>,
    generation: Cell<u64>,
    selected: Cell<Filter>,
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

/// Summary card: icon and counter on the first line, caption below.
struct MetricCard {
    filter: Filter,
    button: gtk4::Button,
    count: gtk4::Label,
    caption: gtk4::Label,
}

impl MetricCard {
    fn new(filter: Filter) -> Self {
        let icon = gtk4::Image::from_icon_name(filter.icon());
        icon.set_pixel_size(20);
        icon.add_css_class("security-metric-icon");
        let count = gtk4::Label::new(Some("0"));
        count.add_css_class("security-metric-count");
        let top = gtk4::Box::new(Orientation::Horizontal, 10);
        top.append(&icon);
        top.append(&count);
        let caption = label(filter.label_key(), "security-metric-caption");
        let content = gtk4::Box::new(Orientation::Vertical, 6);
        content.append(&top);
        content.append(&caption);
        let button = gtk4::Button::new();
        button.set_child(Some(&content));
        button.add_css_class("security-metric");
        Self {
            filter,
            button,
            count,
            caption,
        }
    }

    fn update(&self, count: usize, selected: bool) {
        let caption = tr(self.filter.label_key());
        self.count.set_text(&count.to_string());
        self.caption.set_text(&caption);
        // Also read by screen readers: the card content is split across two labels.
        self.button
            .set_tooltip_text(Some(&format!("{count} {caption}")));
        toggle_class(&self.button, "security-metric-selected", selected);
        toggle_class(&self.button, "security-metric-attention", count > 0);
    }
}

fn toggle_class(widget: &impl IsA<gtk4::Widget>, class: &str, enabled: bool) {
    if enabled {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

/// How urgent a finding is; drives its colour, never its wording.
#[derive(Clone, Copy)]
enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    fn class(self) -> &'static str {
        match self {
            Self::High => "security-severity-high",
            Self::Medium => "security-severity-medium",
            Self::Low => "security-severity-low",
        }
    }
}

fn severity(finding: Finding) -> Severity {
    match finding {
        // Same colours as the list badges: weak is red there, reused is amber.
        Finding::Expired | Finding::WeakPassword => Severity::High,
        Finding::ReusedPassword => Severity::Medium,
        _ => Severity::Low,
    }
}

fn finding_icon(finding: Finding) -> &'static str {
    match finding {
        Finding::WeakPassword => "hv-key-symbolic",
        Finding::ReusedPassword => "hv-reuse-symbolic",
        Finding::Expired | Finding::ExpiringSoon | Finding::InvalidExpiry => "hv-clock-symbolic",
        Finding::Url(_) => "hv-link-symbolic",
        _ => "hv-info-symbolic",
    }
}

/// One summary card per filter, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    All,
    Reused,
    Weak,
    Expiry,
    Urls,
}

impl Filter {
    const ALL: [Self; 5] = [
        Self::All,
        Self::Reused,
        Self::Weak,
        Self::Expiry,
        Self::Urls,
    ];

    fn icon(self) -> &'static str {
        match self {
            Self::All => "hv-shield-symbolic",
            Self::Reused => "hv-reuse-symbolic",
            Self::Weak => "hv-key-symbolic",
            Self::Expiry => "hv-clock-symbolic",
            Self::Urls => "hv-link-symbolic",
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::All => "security-card-all",
            Self::Reused => "security-card-reused",
            Self::Weak => "security-card-weak",
            Self::Expiry => "security-card-expiry",
            Self::Urls => "security-card-urls",
        }
    }

    /// Reuse groups are only meaningful when every member of a group is listed.
    fn groups_reuse(self) -> bool {
        matches!(self, Self::All | Self::Reused)
    }
}

fn matches_filter(entry: &SecurityEntry, filter: Filter) -> bool {
    entry.findings.iter().any(|finding| match filter {
        Filter::All => true,
        Filter::Reused => *finding == Finding::ReusedPassword,
        Filter::Weak => *finding == Finding::WeakPassword,
        Filter::Expiry => matches!(
            finding,
            Finding::Expired | Finding::ExpiringSoon | Finding::InvalidExpiry
        ),
        Filter::Urls => matches!(finding, Finding::Url(_)),
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
        Finding::Unreadable => "security-finding-unreadable",
        // `Finding` is non-exhaustive: an unknown future finding is never shown as healthy.
        _ => "security-finding-unreadable",
    }
}

fn rank(finding: Finding) -> u8 {
    match finding {
        Finding::Unreadable | Finding::InvalidMetadata => 0,
        Finding::Expired => 1,
        Finding::ReusedPassword => 2,
        Finding::WeakPassword => 3,
        Finding::Url(_) => 4,
        _ => 5,
    }
}

/// The finding that sorts the entry and gives its row icon.
fn main_finding(entry: &SecurityEntry) -> Option<Finding> {
    entry
        .findings
        .iter()
        .copied()
        .min_by_key(|&finding| rank(finding))
}

fn priority(entry: &SecurityEntry) -> u8 {
    main_finding(entry).map_or(6, rank)
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
        if let Some(source) = self.pending.borrow_mut().take() {
            source.remove();
        }
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(control) = self.control.borrow_mut().take() {
            control.cancel();
        }
        self.busy.set(false);
        self.spinner.stop();
        self.cancel.set_visible(false);
        self.refresh.set_sensitive(true);
    }

    /// Hero shield and sidebar counter: the at-a-glance state of the last analysis.
    fn render_status(&self, report: Option<&SecurityReport>) {
        let affected = report.map_or(0, SecurityReport::affected);
        let pending = self.pending.borrow().is_some() || self.busy.get();
        let (icon, attention, healthy) = match report {
            _ if pending => ("hv-shield-symbolic", false, false),
            _ if self.failed.get() => ("hv-shield-warning-symbolic", true, false),
            None => ("hv-shield-symbolic", false, false),
            Some(_) if affected > 0 => ("hv-shield-warning-symbolic", true, false),
            Some(report) if !report.is_complete() => ("hv-shield-partial-symbolic", true, false),
            Some(_) => ("hv-shield-ok-symbolic", false, true),
        };
        self.shield.set_icon_name(Some(icon));
        toggle_class(&self.hero, "security-hero-attention", attention);
        toggle_class(&self.hero, "security-hero-ok", healthy);
        let (badge, tooltip) = if pending {
            ("...".to_string(), tr("security-center-updating"))
        } else if self.failed.get() {
            ("!".to_string(), tr("security-center-error-detail"))
        } else {
            match report {
                None => ("?".to_string(), tr("security-center-ready")),
                Some(report) if !report.is_complete() => (
                    if affected > 0 {
                        affected.to_string()
                    } else {
                        "!".to_string()
                    },
                    tr("security-center-partial"),
                ),
                Some(_) => (
                    affected.to_string(),
                    tr_args(
                        "security-center-summary",
                        &[("count", I18nArg::Str(&affected.to_string()))],
                    ),
                ),
            }
        };
        self.nav.count.set_text(&badge);
        self.nav.count.set_visible(true);
        self.nav.button.set_tooltip_text(Some(&tooltip));
        toggle_class(
            &self.nav.count,
            "security-count-pending",
            pending || (report.is_none() && !self.failed.get()),
        );
        toggle_class(&self.nav.count, "security-count-ok", healthy);
    }

    /// Progress tick while a scan runs: only the counter changes, no widget is rebuilt.
    fn render_progress(&self) {
        let count = self
            .control
            .borrow()
            .as_ref()
            .map_or(0, |control| control.processed());
        self.summary.set_text(&tr_args(
            "security-center-progress",
            &[("count", I18nArg::Str(&count.to_string()))],
        ));
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
        for card in &self.cards {
            let count = stored.as_ref().map_or(0, |report| {
                report
                    .entries
                    .iter()
                    .filter(|entry| matches_filter(entry, card.filter))
                    .count()
            });
            card.update(count, card.filter == self.selected.get());
        }
        self.render_status(stored.as_ref());
        if self.pending.borrow().is_some() {
            self.summary.set_text(&tr("security-center-updating"));
            self.coverage.set_text(&tr("security-center-local"));
            return;
        }
        if self.busy.get() {
            self.render_progress();
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
                && self.selected.get().groups_reuse()
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
        let main = main_finding(entry);
        let icon = gtk4::Image::from_icon_name(main.map_or("hv-info-symbolic", finding_icon));
        icon.set_pixel_size(18);
        icon.set_valign(Align::Center);
        icon.add_css_class("security-finding-icon");
        icon.add_css_class(main.map_or(Severity::Low, severity).class());
        header.append(&icon);
        let text = gtk4::Box::new(Orientation::Vertical, 4);
        text.set_hexpand(true);
        let title = gtk4::Label::new(Some(&entry.title));
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.add_css_class("heading");
        text.append(&title);
        let meta = gtk4::Box::new(Orientation::Horizontal, 6);
        let vault = gtk4::Label::new(Some(&entry.vault_name));
        vault.set_xalign(0.0);
        vault.add_css_class("dim-label");
        meta.append(&vault);
        for &finding in &entry.findings {
            let chip = gtk4::Label::new(Some(&tr(short_finding_key(finding))));
            chip.add_css_class("security-chip");
            chip.add_css_class(severity(finding).class());
            meta.append(&chip);
        }
        text.append(&meta);
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
    pub navigation: super::SecurityNavigation,
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
        hero: gtk4::Box::new(Orientation::Horizontal, 20),
        shield: gtk4::Image::from_icon_name("hv-shield-symbolic"),
        nav: deps.navigation.clone(),
        cards: Filter::ALL.into_iter().map(MetricCard::new).collect(),
        vaults: gtk4::DropDown::from_strings(&[&tr("security-center-vaults")]),
        results: gtk4::Box::new(Orientation::Vertical, 12),
        more: gtk4::Button::with_label(&tr("security-center-more")),
        limit: Cell::new(50),
        report: RefCell::new(None),
        control: RefCell::new(None),
        pending: RefCell::new(None),
        generation: Cell::new(0),
        selected: Cell::new(Filter::All),
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
    let hero = view.hero.clone();
    hero.add_css_class("security-hero");
    view.shield.set_pixel_size(64);
    view.shield.add_css_class("security-hero-shield");
    hero.append(&view.shield);
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
    for card in &view.cards {
        cards.insert(&card.button, -1);
        let filter = card.filter;
        let weak = Rc::downgrade(&view);
        card.button.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.selected.set(filter);
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
            if view.busy.get() {
                return;
            }
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
                view.render_progress();
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
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "Security center analysis failed");
                        view.failed.set(true);
                        view.render();
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Security center analysis worker failed");
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
    let new_pill = deps.navigation.new_pill.clone();
    deps.navigation.button.connect_clicked(move |_| {
        if new_pill.is_visible() {
            new_pill.set_visible(false);
            crate::ui::view_preferences::persist_security_center_seen();
        }
        stack.set_visible_child_name("security_center_view");
        if weak
            .upgrade()
            .is_some_and(|view| !view.busy.get() && view.report.borrow().is_none())
        {
            start();
        }
    });
    let nav_button = deps.navigation.button.clone();
    deps.stack.connect_visible_child_name_notify(move |stack| {
        let active = stack.visible_child_name().as_deref() == Some("security_center_view");
        toggle_class(&nav_button, "security-navigation-active", active);
    });
    view.render();
    let refresh_i18n = {
        let view = Rc::clone(&view);
        let stack = deps.stack.clone();
        let page = page.clone();
        Rc::new(move || {
            stack.page(&page).set_title(&tr("security-center-title"));
            view.render();
            if let Some(model) = view.vaults.model().and_downcast::<gtk4::StringList>() {
                model.splice(0, 1, &[&tr("security-center-vaults")]);
            }
        }) as Rc<dyn Fn()>
    };
    let invalidate = {
        let view = Rc::downgrade(&view);
        let key = Rc::clone(&deps.key);
        Rc::new(move || {
            let Some(view) = view.upgrade() else { return };
            view.stop();
            *view.report.borrow_mut() = None;
            view.failed.set(false);
            if !key.borrow().is_empty() {
                // Coalesce import/refresh bursts without delaying invalidation of old results.
                let weak = Rc::downgrade(&view);
                let scan = Rc::clone(&scan);
                *view.pending.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(250),
                    move || {
                        if let Some(view) = weak.upgrade() {
                            view.pending.borrow_mut().take();
                            scan();
                        }
                    },
                ));
            }
            view.render();
        }) as Rc<dyn Fn()>
    };
    let view_for_clear = Rc::clone(&view);
    let clear = Rc::new(move || {
        let view = &view_for_clear;
        view.stop();
        *view.report.borrow_mut() = None;
        view.failed.set(false);
        *view.completed_at.borrow_mut() = None;
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
pub(super) mod tests {
    use super::*;
    use heelonvault_core::models::{SecretType, UserRole};
    use heelonvault_core::repositories::user_repository::UserRepository;
    use heelonvault_core::repositories::{
        secret_repository::SqlxSecretRepository, team_repository::SqlxTeamRepository,
        user_repository::SqlxUserRepository, vault_repository::SqlxVaultRepository,
    };
    use heelonvault_core::services::{
        audit_log_service::NoOpAuditLogService, crypto_service::CryptoServiceImpl,
        secret_service::SecretServiceImpl, vault_service::VaultServiceImpl,
    };

    fn fixture_entry(
        title: &str,
        vault_id: Uuid,
        vault_name: &str,
        findings: Vec<Finding>,
        reuse_group: Option<usize>,
    ) -> SecurityEntry {
        let mut entry = SecurityEntry::default();
        entry.secret_id = Uuid::new_v4();
        entry.vault_id = vault_id;
        entry.title = title.to_string();
        entry.vault_name = vault_name.to_string();
        entry.findings = findings;
        entry.reuse_group = reuse_group;
        entry
    }

    #[test]
    fn filters_and_priorities_are_explicit() {
        let mut entry = fixture_entry(
            "",
            Uuid::nil(),
            "",
            vec![Finding::WeakPassword, Finding::ReusedPassword],
            Some(0),
        );
        assert!(matches_filter(&entry, Filter::All));
        assert!(matches_filter(&entry, Filter::Reused));
        assert!(matches_filter(&entry, Filter::Weak));
        assert!(!matches_filter(&entry, Filter::Expiry));
        assert!(!matches_filter(&entry, Filter::Urls));
        assert_eq!(priority(&entry), 2);
        entry.findings.push(Finding::Unreadable);
        assert_eq!(priority(&entry), 0);
    }

    /// Run by `native_gtk_smoke` (main_window/mod.rs), on the thread that initialised GTK.
    pub(in crate::ui::windows::main_window) fn native_security_center_scenario()
    -> Result<(), Box<dyn std::error::Error>> {
        let display = gtk4::gdk::Display::default().ok_or("no graphical display")?;
        // Every stylesheet must parse cleanly: GTK silently drops a rule it cannot read.
        let css_errors = Rc::new(RefCell::new(Vec::new()));
        // accent-modern.css is only loaded on libadwaita >= 1.6 (see ui/theme.rs).
        let modern = adw::major_version() > 1 || adw::minor_version() >= 6;
        let stylesheets: &[&str] = if modern {
            &["style.css", "style/accent-modern.css"]
        } else {
            &["style.css"]
        };
        for &path in stylesheets {
            let probe = gtk4::CssProvider::new();
            let errors = Rc::clone(&css_errors);
            probe.connect_parsing_error(move |_, section, error| {
                errors
                    .borrow_mut()
                    .push(format!("{path}: {section}: {error}"));
            });
            probe.load_from_resource(&format!("/com/heelonvault/rust/{path}"));
        }
        assert!(css_errors.borrow().is_empty(), "{:?}", css_errors.borrow());
        crate::ui::theme::install(&display);
        let icons = gtk4::IconTheme::for_display(&display);
        for name in [
            "hv-shield-ok-symbolic",
            "hv-reuse-symbolic",
            "hv-link-symbolic",
        ] {
            assert!(icons.has_icon(name), "bundled icon {name} not found");
        }
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
        let pool = runtime.block_on(async {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await?;
            sqlx::migrate::Migrator::new(std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/migrations"
            )))
            .await?
            .run(&pool)
            .await?;
            Ok::<_, Box<dyn std::error::Error>>(pool)
        })?;
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
            SqlxTeamRepository::new(pool.clone()),
            Arc::new(NoOpAuditLogService),
            CryptoServiceImpl::default(),
        ));
        let user_id = Uuid::new_v4();
        runtime.block_on(SqlxUserRepository::new(pool.clone()).create_user_db(
            user_id,
            "security-test",
            &UserRole::Admin,
        ))?;
        let key = Rc::new(RefCell::new(vec![1; 32]));
        let center = build(CenterDeps {
            runtime: runtime.handle().clone(),
            secrets: Arc::clone(&secret_service),
            vaults: Arc::clone(&vault_service),
            user_id,
            key: Rc::clone(&key),
            stack: panel.main_stack.clone(),
            navigation: sidebar.security_nav.clone(),
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
        .map(|(title, findings, reuse_group)| {
            fixture_entry(title, vault_id, "Personnel", findings, reuse_group)
        })
        .collect();
        let mut report = SecurityReport::default();
        report.entries = entries;
        report.analyzed = 124;
        report.vaults_analyzed = 3;
        center.view.set_report(report);
        assert_eq!(
            center
                .view
                .report
                .borrow()
                .as_ref()
                .map(SecurityReport::affected),
            Some(4)
        );
        center.view.cards[4].button.emit_clicked();
        assert_eq!(center.view.selected.get(), Filter::Urls);
        assert!(
            center
                .view
                .results
                .first_child()
                .is_some_and(|c| c.next_sibling().is_none())
        );
        center.view.cards[0].button.emit_clicked();
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
            .map(|_| {
                fixture_entry(
                    "Synthetic repeated item",
                    vault_id,
                    "Fixture",
                    vec![Finding::ReusedPassword],
                    Some(0),
                )
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
        let mut partial = SecurityReport::default();
        partial.unavailable_vaults = 1;
        center.view.set_report(partial);
        assert_eq!(sidebar.security_nav.count.text(), "!");
        assert!(
            !sidebar
                .security_nav
                .count
                .has_css_class("security-count-ok")
        );

        // The same invalidation used by login and mutation refreshes must work off-page.
        panel.main_stack.set_visible_child_name("entries_view");
        (center.invalidate)();
        assert_eq!(sidebar.security_nav.count.text(), "...");
        assert!(center.view.report.borrow().is_none());
        let wait_for_report = || -> Result<(), Box<dyn std::error::Error>> {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while center.view.pending.borrow().is_some() || center.view.busy.get() {
                if std::time::Instant::now() >= deadline {
                    return Err("security analysis did not finish within five seconds".into());
                }
                context.iteration(false);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(!center.view.failed.get());
            Ok(())
        };
        wait_for_report()?;
        assert_eq!(sidebar.security_nav.count.text(), "0");
        assert!(
            sidebar
                .security_nav
                .count
                .has_css_class("security-count-ok")
        );
        assert_eq!(
            panel.main_stack.visible_child_name().as_deref(),
            Some("entries_view")
        );

        let (real_vault, secret) = runtime.block_on(async {
            let vault = vault_service
                .create_vault(user_id, "Live vault", SecretBox::new(Box::new(vec![1; 32])))
                .await?;
            let vault_key = vault_service
                .open_vault_for_user(user_id, vault.id, SecretBox::new(Box::new(vec![1; 32])))
                .await?;
            let secret = secret_service
                .create_secret(
                    vault.id,
                    SecretType::Password,
                    Some("Weak password".to_string()),
                    None,
                    None,
                    None,
                    SecretBox::new(Box::new(b"Password123!".to_vec())),
                    vault_key,
                )
                .await?;
            Ok::<_, Box<dyn std::error::Error>>((vault, secret))
        })?;
        for _ in 0..5 {
            (center.invalidate)();
        }
        let scheduled_generation = center.view.generation.get();
        assert!(
            !center.view.busy.get(),
            "refresh bursts should be debounced"
        );
        wait_for_report()?;
        assert_eq!(
            center.view.generation.get(),
            scheduled_generation + 2,
            "one scan start and one completion, not five scans"
        );
        assert_eq!(sidebar.security_nav.count.text(), "1");
        assert!(
            !sidebar
                .security_nav
                .count
                .has_css_class("security-count-ok")
        );
        let generation = center.view.generation.get();
        sidebar.security_nav.button.emit_clicked();
        assert_eq!(
            center.view.generation.get(),
            generation,
            "fresh results are reused"
        );
        center.view.refresh.emit_clicked();
        assert!(center.view.busy.get());
        panel.main_stack.set_visible_child_name("entries_view");
        drain();
        wait_for_report()?;
        assert!(
            center.view.report.borrow().is_some(),
            "leaving the page must not cancel analysis"
        );

        runtime.block_on(secret_service.soft_delete(secret.id))?;
        (center.invalidate)();
        wait_for_report()?;
        assert_eq!(sidebar.security_nav.count.text(), "0");
        runtime.block_on(secret_service.restore_secret(secret.id, real_vault.id))?;
        (center.invalidate)();
        wait_for_report()?;
        assert_eq!(sidebar.security_nav.count.text(), "1");

        // Cancellation/locking must invalidate results, including a queued scan.
        (center.invalidate)();
        assert!(center.view.pending.borrow().is_some());
        (center.clear)();
        drain();
        assert!(center.view.report.borrow().is_none());
        assert!(!center.view.busy.get());
        assert!(center.view.pending.borrow().is_none());
        assert_eq!(sidebar.security_nav.count.text(), "?");
        sidebar.security_nav.button.emit_clicked();
        assert!(
            center.view.busy.get(),
            "opening without a report starts analysis"
        );
        center.view.cancel.emit_clicked();
        drain();
        assert!(center.view.report.borrow().is_none());
        assert!(!center.view.busy.get());
        assert_eq!(sidebar.security_nav.count.text(), "?");
        center.view.refresh.emit_clicked();
        assert!(center.view.busy.get());
        key.borrow_mut().clear();
        (center.clear)();
        drain();
        assert!(
            center.view.report.borrow().is_none(),
            "late results after locking are ignored"
        );
        (center.invalidate)();
        assert!(
            center.view.pending.borrow().is_none(),
            "locked sessions never schedule scans"
        );

        *key.borrow_mut() = vec![1; 32];
        runtime.block_on(sqlx::query("DROP TABLE vaults").execute(&pool))?;
        (center.invalidate)();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !center.view.failed.get() {
            if std::time::Instant::now() >= deadline {
                return Err("analysis error was not surfaced".into());
            }
            context.iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(sidebar.security_nav.count.text(), "!");
        assert!(
            !sidebar
                .security_nav
                .count
                .has_css_class("security-count-ok")
        );
        (center.clear)();
        window.close();
        drain();
        Ok(())
    }
}

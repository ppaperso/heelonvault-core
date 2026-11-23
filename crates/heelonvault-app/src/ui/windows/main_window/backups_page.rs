//! Backups page: overall status, destinations (local folder and mirror folders), schedule,
//! database health, restore test and run history. Reads the settings and the journal kept by
//! `crate::ui::auto_backup`; every slow operation (backup, mirror check, restore test) runs
//! off the GTK thread.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use heelonvault_core::i18n::{I18nArg, tr_args};
use heelonvault_core::services::auto_backup::{
    AutoBackupSettings, BackupRunRecord, BackupSchedule, BackupTrigger, DatabaseHealth,
    DestinationState, MirrorStatus, RunOutcome,
};
use libadwaita as adw;
use libadwaita::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ui::auto_backup::{self, BackupStatus};

/// Name of the page in the main stack.
pub(super) const PAGE_NAME: &str = "backups_view";
/// Runs listed in the history.
const HISTORY_ROWS: usize = 10;

/// The schedule as two independent switches: daily at unlock, and on close.
fn schedule_from(daily: bool, on_close: bool) -> BackupSchedule {
    match (daily, on_close) {
        (true, true) => BackupSchedule::DailyAndOnClose,
        (true, false) => BackupSchedule::Daily,
        (false, true) => BackupSchedule::OnClose,
        (false, false) => BackupSchedule::Off,
    }
}

fn tr(key: &str) -> String {
    heelonvault_core::tr!(key)
}

fn num(value: u64) -> I18nArg<'static> {
    I18nArg::Num(i64::try_from(value).unwrap_or(i64::MAX))
}

fn local_date(instant: OffsetDateTime) -> String {
    crate::ui::local_time::to_local(instant)
        .and_then(|local| local.format("%x %H:%M").ok())
        .map(|text| text.to_string())
        .unwrap_or_default()
}

fn size(bytes: u64) -> String {
    gtk4::glib::format_size(bytes).to_string()
}

fn trigger_key(trigger: BackupTrigger) -> &'static str {
    match trigger {
        BackupTrigger::Unlock => "backups-trigger-unlock",
        BackupTrigger::Close => "backups-trigger-close",
        BackupTrigger::PasswordChange => "backups-trigger-password-change",
        _ => "backups-trigger-manual",
    }
}

/// Status title, icon and CSS class.
fn status_visual(status: BackupStatus) -> (&'static str, &'static str, &'static str) {
    match status {
        BackupStatus::UpToDate => (
            "backups-status-up-to-date",
            "hv-shield-ok-symbolic",
            "backups-status-ok",
        ),
        BackupStatus::Disabled => (
            "backups-status-disabled",
            "hv-shield-partial-symbolic",
            "backups-status-neutral",
        ),
        BackupStatus::Never => (
            "backups-status-never",
            "hv-shield-partial-symbolic",
            "backups-status-warning",
        ),
        BackupStatus::Overdue => (
            "backups-status-overdue",
            "hv-shield-warning-symbolic",
            "backups-status-warning",
        ),
        BackupStatus::MirrorMissed => (
            "backups-status-mirror",
            "hv-shield-warning-symbolic",
            "backups-status-warning",
        ),
        BackupStatus::Failed => (
            "backups-status-failed",
            "hv-shield-warning-symbolic",
            "backups-status-error",
        ),
    }
}

fn pill_key(status: BackupStatus) -> &'static str {
    match status {
        BackupStatus::Failed => "backups-pill-failed",
        BackupStatus::MirrorMissed => "backups-pill-mirror",
        _ => "backups-pill-overdue",
    }
}

fn mirror_state_key(state: &DestinationState) -> &'static str {
    match state {
        DestinationState::Ready => "backups-mirror-ready",
        DestinationState::Missing => "backups-mirror-missing",
        DestinationState::NotADirectory => "backups-mirror-not-directory",
        _ => "backups-mirror-unreachable",
    }
}

/// One-line outcome of a run for the history.
fn outcome_text(record: &BackupRunRecord) -> String {
    match record.outcome {
        RunOutcome::Success => tr("backups-outcome-success"),
        RunOutcome::Partial => tr("backups-outcome-partial"),
        // A refused damaged database is the one failure worth naming.
        _ if record
            .health
            .as_ref()
            .is_some_and(|health| !health.is_healthy()) =>
        {
            tr("backups-outcome-damaged")
        }
        _ => tr("backups-outcome-failed"),
    }
}

fn history_detail(record: &BackupRunRecord) -> String {
    let mirrors = if record.mirrors.is_empty() {
        tr("backups-history-no-mirror")
    } else {
        let copied = record
            .mirrors
            .iter()
            .filter(|mirror| mirror.status == MirrorStatus::Copied)
            .count();
        tr_args(
            "backups-history-mirrors",
            &[
                ("copied", num(copied as u64)),
                ("total", num(record.mirrors.len() as u64)),
            ],
        )
    };
    let mut parts = vec![outcome_text(record)];
    if record.succeeded() {
        parts.push(size(record.size));
        parts.push(mirrors);
    }
    parts.join(" · ")
}

fn row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_subtitle_selectable(false);
    row
}

fn switch_row() -> (adw::ActionRow, gtk4::Switch) {
    let row = row("", "");
    let switch = gtk4::Switch::new();
    switch.set_valign(Align::Center);
    row.add_suffix(&switch);
    row.set_activatable_widget(Some(&switch));
    (row, switch)
}

fn suffix_button(label: &str) -> gtk4::Button {
    let button = gtk4::Button::with_label(label);
    button.add_css_class("flat");
    button.set_valign(Align::Center);
    button
}

struct View {
    window: adw::ApplicationWindow,
    toast: adw::ToastOverlay,
    user_id: Uuid,
    key: Rc<RefCell<Vec<u8>>>,
    nav_pill: gtk4::Label,
    title: gtk4::Label,
    subtitle: gtk4::Label,
    status_icon: gtk4::Image,
    status_card: gtk4::Box,
    status_title: gtk4::Label,
    status_detail: gtk4::Label,
    backup_now: gtk4::Button,
    spinner: gtk4::Spinner,
    destinations: adw::PreferencesGroup,
    local_row: adw::ActionRow,
    local_open: gtk4::Button,
    add_mirror_row: adw::ActionRow,
    add_mirror: gtk4::Button,
    mirror_rows: RefCell<Vec<adw::ActionRow>>,
    schedule_group: adw::PreferencesGroup,
    daily_row: adw::ActionRow,
    daily_switch: gtk4::Switch,
    close_row: adw::ActionRow,
    close_switch: gtk4::Switch,
    retention_row: adw::ActionRow,
    syncing_schedule: Cell<bool>,
    health_group: adw::PreferencesGroup,
    health_rows: [adw::ActionRow; 5],
    restore_row: adw::ActionRow,
    restore_button: gtk4::Button,
    history_group: adw::PreferencesGroup,
    history_rows: RefCell<Vec<adw::ActionRow>>,
}

impl View {
    fn session_key(&self) -> Vec<u8> {
        self.key.borrow().clone()
    }

    fn refresh_i18n(&self) {
        self.title.set_text(&tr("backups-title"));
        self.subtitle.set_text(&tr("backups-subtitle"));
        self.backup_now.set_label(&tr("backups-now"));
        self.destinations
            .set_title(&tr("backups-destinations-title"));
        self.destinations
            .set_description(Some(&tr("backups-destinations-description")));
        self.local_row.set_title(&tr("backups-local-title"));
        self.local_open.set_label(&tr("backups-open-folder"));
        self.add_mirror_row
            .set_title(&tr("backups-mirror-add-title"));
        self.add_mirror_row
            .set_subtitle(&tr("backups-mirror-add-subtitle"));
        self.add_mirror.set_label(&tr("backups-mirror-add"));
        self.schedule_group.set_title(&tr("backups-schedule-title"));
        self.schedule_group
            .set_description(Some(&tr("backups-schedule-description")));
        self.daily_row.set_title(&tr("backups-schedule-daily"));
        self.daily_row
            .set_subtitle(&tr("backups-schedule-daily-subtitle"));
        self.close_row.set_title(&tr("backups-schedule-close"));
        self.close_row
            .set_subtitle(&tr("backups-schedule-close-subtitle"));
        self.retention_row.set_title(&tr("backups-retention-title"));
        self.health_group.set_title(&tr("backups-health-title"));
        for (row, key) in self.health_rows.iter().zip([
            "backups-health-integrity",
            "backups-health-references",
            "backups-health-content",
            "backups-health-size",
            "backups-health-schema",
        ]) {
            row.set_title(&tr(key));
        }
        self.restore_row
            .set_title(&tr("backups-restore-test-title"));
        self.restore_row
            .set_subtitle(&tr("backups-restore-test-subtitle"));
        self.restore_button.set_label(&tr("backups-restore-test"));
        self.history_group.set_title(&tr("backups-history-title"));
        self.render();
    }

    /// Re-reads the settings and the journal and updates every section.
    fn render(&self) {
        let settings = auto_backup::load_settings();
        let journal = auto_backup::journal();
        let now = OffsetDateTime::now_utc();
        let status = auto_backup::status(&settings, &journal, now);
        let running = auto_backup::is_running();

        let (title_key, icon, class) = status_visual(status);
        for other in [
            "backups-status-ok",
            "backups-status-neutral",
            "backups-status-warning",
            "backups-status-error",
        ] {
            self.status_card.remove_css_class(other);
        }
        self.status_card.add_css_class(class);
        self.status_icon.set_icon_name(Some(icon));
        self.status_title.set_text(&if running {
            tr("backups-running")
        } else {
            tr(title_key)
        });
        self.status_detail.set_text(&match journal.last_success() {
            Some(run) => tr_args(
                "backups-last-success",
                &[("date", I18nArg::Str(&local_date(run.finished_at)))],
            ),
            None => tr("backups-no-success"),
        });
        self.backup_now.set_sensitive(!running);
        self.spinner.set_visible(running);
        self.spinner.set_spinning(running);
        self.nav_pill.set_visible(status.needs_attention());
        self.nav_pill.set_text(&tr(pill_key(status)));

        let local = auto_backup::backup_dir().unwrap_or_default();
        self.local_row.set_subtitle(&local.display().to_string());
        self.render_mirrors(&settings);

        self.syncing_schedule.set(true);
        self.daily_switch.set_active(settings.schedule.on_unlock());
        self.close_switch.set_active(settings.schedule.on_close());
        self.syncing_schedule.set(false);
        self.retention_row.set_subtitle(&tr_args(
            "backups-retention-value",
            &[
                ("daily", num(u64::from(settings.retention.daily))),
                ("weekly", num(u64::from(settings.retention.weekly))),
                ("monthly", num(u64::from(settings.retention.monthly))),
            ],
        ));

        self.render_health(journal.last_success().and_then(|run| run.health.as_ref()));
        self.render_history(&journal.runs);
    }

    fn render_health(&self, health: Option<&DatabaseHealth>) {
        let [integrity, references, content, size_row, schema] = &self.health_rows;
        let Some(health) = health else {
            for row in &self.health_rows {
                row.set_subtitle(&tr("backups-health-empty"));
            }
            return;
        };
        integrity.set_subtitle(&if health.integrity_ok {
            tr("backups-ok")
        } else {
            tr_args(
                "backups-integrity-issues",
                &[("count", num(u64::from(health.integrity_issues)))],
            )
        });
        references.set_subtitle(&if health.foreign_key_violations == 0 {
            tr("backups-ok")
        } else {
            tr_args(
                "backups-references-issues",
                &[("count", num(u64::from(health.foreign_key_violations)))],
            )
        });
        content.set_subtitle(&tr_args(
            "backups-health-content-value",
            &[
                ("secrets", num(health.secret_count)),
                ("vaults", num(health.vault_count)),
                ("users", num(health.user_count)),
            ],
        ));
        size_row.set_subtitle(&tr_args(
            "backups-health-size-value",
            &[
                ("database", I18nArg::Str(&size(health.database_bytes))),
                ("wal", I18nArg::Str(&size(health.wal_bytes))),
            ],
        ));
        schema.set_subtitle(
            &health
                .schema_version
                .map(|version| version.to_string())
                .unwrap_or_else(|| "-".to_string()),
        );
    }

    fn render_history(&self, runs: &[BackupRunRecord]) {
        for old in self.history_rows.borrow_mut().drain(..) {
            self.history_group.remove(&old);
        }
        let mut rows = Vec::new();
        if runs.is_empty() {
            let empty = row(&tr("backups-history-empty"), "");
            self.history_group.add(&empty);
            rows.push(empty);
        }
        for record in runs.iter().rev().take(HISTORY_ROWS) {
            let entry = row(
                &format!(
                    "{} · {}",
                    local_date(record.finished_at),
                    tr(trigger_key(record.trigger))
                ),
                &history_detail(record),
            );
            let (icon, class) = match record.outcome {
                RunOutcome::Success => ("hv-check-symbolic", "backups-run-ok"),
                RunOutcome::Partial => ("hv-warning-symbolic", "backups-run-warning"),
                _ => ("hv-warning-symbolic", "backups-run-error"),
            };
            let image = gtk4::Image::from_icon_name(icon);
            image.add_css_class(class);
            entry.add_prefix(&image);
            entry.add_css_class("backups-history-row");
            self.history_group.add(&entry);
            rows.push(entry);
        }
        *self.history_rows.borrow_mut() = rows;
    }

    fn render_mirrors(&self, settings: &AutoBackupSettings) {
        for old in self.mirror_rows.borrow_mut().drain(..) {
            self.destinations.remove(&old);
        }
        // Keep "Add a mirror folder" last.
        self.destinations.remove(&self.add_mirror_row);
        let mut rows = Vec::new();
        for mirror in &settings.mirrors {
            let entry = row(
                &mirror.display().to_string(),
                &tr("backups-mirror-checking"),
            );
            entry.add_css_class("backups-mirror-row");
            let icon = gtk4::Image::from_icon_name("hv-link-symbolic");
            entry.add_prefix(&icon);
            let remove = gtk4::Button::from_icon_name("hv-trash-symbolic");
            remove.add_css_class("flat");
            remove.set_valign(Align::Center);
            remove.set_tooltip_text(Some(&tr("backups-mirror-remove")));
            let path = mirror.clone();
            let toast = self.toast.clone();
            remove.connect_clicked(move |button| {
                let mut settings = auto_backup::load_settings();
                settings.mirrors.retain(|existing| existing != &path);
                save_or_toast(&settings, &toast);
                if let Err(error) = button.activate_action("win.backups-refresh", None) {
                    tracing::debug!(%error, "backups refresh action unavailable");
                }
            });
            entry.add_suffix(&remove);
            self.destinations.add(&entry);
            rows.push(entry);
        }
        self.destinations.add(&self.add_mirror_row);
        if !settings.mirrors.is_empty() {
            let targets: Vec<(PathBuf, adw::ActionRow)> = settings
                .mirrors
                .iter()
                .cloned()
                .zip(rows.iter().cloned())
                .collect();
            auto_backup::check_mirrors(settings.mirrors.clone(), move |states| {
                for ((path, entry), (_, state)) in targets.iter().zip(states.iter()) {
                    entry.set_subtitle(&tr(mirror_state_key(state)));
                    let ready = *state == DestinationState::Ready;
                    if ready {
                        entry.remove_css_class("backups-mirror-unavailable");
                    } else {
                        entry.add_css_class("backups-mirror-unavailable");
                    }
                    tracing::debug!(mirror = %path.display(), ready, "mirror checked");
                }
            });
        }
        *self.mirror_rows.borrow_mut() = rows;
    }
}

fn save_or_toast(settings: &AutoBackupSettings, toast: &adw::ToastOverlay) {
    if let Err(error) = auto_backup::save_settings(settings) {
        tracing::warn!(%error, "backup settings could not be saved");
        toast.add_toast(adw::Toast::new(&tr("backups-settings-save-failed")));
    }
}

pub(super) struct BackupsDeps {
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub stack: gtk4::Stack,
    pub nav_button: gtk4::Button,
    pub nav_pill: gtk4::Label,
    pub user_id: Uuid,
    pub key: Rc<RefCell<Vec<u8>>>,
    pub editor_navigation: super::editor_navigation::EditorNavigation,
    /// Only administrators back up (the backup covers the whole database).
    pub visible: bool,
}

pub(super) struct BackupsPage {
    pub refresh_i18n: Rc<dyn Fn()>,
}

pub(super) fn build(deps: BackupsDeps) -> BackupsPage {
    let page = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let clamp = adw::Clamp::builder().maximum_size(860).build();
    let root = gtk4::Box::new(Orientation::Vertical, 18);
    root.add_css_class("backups-page");
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(16);
    root.set_margin_end(16);
    clamp.set_child(Some(&root));
    page.set_child(Some(&clamp));
    deps.stack
        .add_titled(&page, Some(PAGE_NAME), &tr("backups-title"));

    let title = gtk4::Label::new(None);
    title.add_css_class("title-2");
    title.set_halign(Align::Start);
    let subtitle = gtk4::Label::new(None);
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(Align::Start);
    subtitle.set_wrap(true);
    subtitle.set_xalign(0.0);
    root.append(&title);
    root.append(&subtitle);

    // Status card: icon, state, last verified backup, "Back up now".
    let status_card = gtk4::Box::new(Orientation::Horizontal, 14);
    status_card.add_css_class("card");
    status_card.add_css_class("backups-status");
    let status_icon = gtk4::Image::new();
    status_icon.set_pixel_size(32);
    status_icon.set_valign(Align::Center);
    let status_text = gtk4::Box::new(Orientation::Vertical, 2);
    status_text.set_hexpand(true);
    status_text.set_valign(Align::Center);
    let status_title = gtk4::Label::new(None);
    status_title.add_css_class("heading");
    status_title.set_halign(Align::Start);
    let status_detail = gtk4::Label::new(None);
    status_detail.add_css_class("dim-label");
    status_detail.set_halign(Align::Start);
    status_detail.set_wrap(true);
    status_detail.set_xalign(0.0);
    status_text.append(&status_title);
    status_text.append(&status_detail);
    let spinner = gtk4::Spinner::new();
    spinner.set_visible(false);
    let backup_now = gtk4::Button::new();
    backup_now.add_css_class("suggested-action");
    backup_now.add_css_class("pill");
    backup_now.set_valign(Align::Center);
    status_card.append(&status_icon);
    status_card.append(&status_text);
    status_card.append(&spinner);
    status_card.append(&backup_now);
    root.append(&status_card);

    let destinations = adw::PreferencesGroup::new();
    let local_row = row("", "");
    local_row.add_prefix(&gtk4::Image::from_icon_name("hv-save-symbolic"));
    let local_open = suffix_button("");
    local_row.add_suffix(&local_open);
    destinations.add(&local_row);
    let add_mirror_row = row("", "");
    add_mirror_row.add_prefix(&gtk4::Image::from_icon_name("hv-add-symbolic"));
    let add_mirror = suffix_button("");
    add_mirror_row.add_suffix(&add_mirror);
    destinations.add(&add_mirror_row);
    root.append(&destinations);

    let schedule_group = adw::PreferencesGroup::new();
    let (daily_row, daily_switch) = switch_row();
    let (close_row, close_switch) = switch_row();
    let retention_row = row("", "");
    schedule_group.add(&daily_row);
    schedule_group.add(&close_row);
    schedule_group.add(&retention_row);
    root.append(&schedule_group);

    let health_group = adw::PreferencesGroup::new();
    let health_rows = [
        row("", ""),
        row("", ""),
        row("", ""),
        row("", ""),
        row("", ""),
    ];
    for entry in &health_rows {
        health_group.add(entry);
    }
    let restore_row = row("", "");
    let restore_button = suffix_button("");
    restore_row.add_suffix(&restore_button);
    health_group.add(&restore_row);
    root.append(&health_group);

    let history_group = adw::PreferencesGroup::new();
    root.append(&history_group);

    deps.nav_button.set_visible(deps.visible);
    let view = Rc::new(View {
        window: deps.window.clone(),
        toast: deps.toast.clone(),
        user_id: deps.user_id,
        key: Rc::clone(&deps.key),
        nav_pill: deps.nav_pill.clone(),
        title,
        subtitle,
        status_icon,
        status_card,
        status_title,
        status_detail,
        backup_now,
        spinner,
        destinations,
        local_row,
        local_open,
        add_mirror_row,
        add_mirror,
        mirror_rows: RefCell::new(Vec::new()),
        schedule_group,
        daily_row,
        daily_switch,
        close_row,
        close_switch,
        retention_row,
        syncing_schedule: Cell::new(false),
        health_group,
        health_rows,
        restore_row,
        restore_button,
        history_group,
        history_rows: RefCell::new(Vec::new()),
    });

    // Rows (mirror removal) ask for a re-render through this action.
    let refresh_action = gtk4::gio::SimpleAction::new("backups-refresh", None);
    {
        let weak = Rc::downgrade(&view);
        refresh_action.connect_activate(move |_, _| {
            if let Some(view) = weak.upgrade() {
                view.render();
            }
        });
    }
    deps.window.add_action(&refresh_action);

    {
        let weak = Rc::downgrade(&view);
        view.backup_now.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            let key = view.session_key();
            let weak = Rc::downgrade(&view);
            auto_backup::request(view.user_id, &key, BackupTrigger::Manual, move |record| {
                if let Some(view) = weak.upgrade() {
                    if let Some(record) = record {
                        view.toast
                            .add_toast(adw::Toast::new(&auto_backup::toast_message(&record)));
                    }
                    view.render();
                }
            });
            // Shows the running state at once.
            view.render();
        });
    }
    {
        let weak = Rc::downgrade(&view);
        view.local_open.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            let Some(dir) = auto_backup::backup_dir().filter(|dir| dir.is_dir()) else {
                return;
            };
            let uri = gtk4::gio::File::for_path(dir).uri();
            gtk4::show_uri(Some(&view.window), &uri, gtk4::gdk::CURRENT_TIME);
        });
    }
    {
        let weak = Rc::downgrade(&view);
        view.add_mirror.connect_clicked(move |_| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            let chooser = gtk4::FileChooserNative::builder()
                .title(tr("backups-mirror-choose-title").as_str())
                .transient_for(&view.window)
                .action(gtk4::FileChooserAction::SelectFolder)
                .modal(true)
                .build();
            let weak = Rc::downgrade(&view);
            chooser.connect_response(move |chooser, response| {
                if response == gtk4::ResponseType::Accept
                    && let Some(path) = chooser.file().and_then(|file| file.path())
                    && let Some(view) = weak.upgrade()
                {
                    let mut settings = auto_backup::load_settings();
                    if !settings.mirrors.contains(&path) {
                        settings.mirrors.push(path);
                        save_or_toast(&settings, &view.toast);
                    }
                    view.render();
                }
                chooser.destroy();
            });
            chooser.show();
        });
    }
    for switch in [&view.daily_switch, &view.close_switch] {
        let weak = Rc::downgrade(&view);
        switch.connect_active_notify(move |_| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            if view.syncing_schedule.get() {
                return;
            }
            let schedule =
                schedule_from(view.daily_switch.is_active(), view.close_switch.is_active());
            let mut settings = auto_backup::load_settings();
            if settings.schedule != schedule {
                settings.schedule = schedule;
                save_or_toast(&settings, &view.toast);
                view.render();
            }
        });
    }
    {
        let weak = Rc::downgrade(&view);
        view.restore_button.connect_clicked(move |button| {
            let Some(view) = weak.upgrade() else {
                return;
            };
            button.set_sensitive(false);
            let key = view.session_key();
            let weak = Rc::downgrade(&view);
            let button = button.clone();
            auto_backup::test_latest_restore(&key, move |result| {
                button.set_sensitive(true);
                let Some(view) = weak.upgrade() else {
                    return;
                };
                let message = match result {
                    Ok((file, health)) if health.is_healthy() => tr_args(
                        "backups-restore-test-ok",
                        &[
                            ("secrets", num(health.secret_count)),
                            ("date", I18nArg::Str(&local_date(file.created_at))),
                        ],
                    ),
                    Ok(_) => tr("backups-restore-test-failed"),
                    Err(()) if auto_backup::journal().last_success().is_none() => {
                        tr("backups-restore-test-none")
                    }
                    Err(()) => tr("backups-restore-test-failed"),
                };
                view.restore_row.set_subtitle(&message);
                view.toast.add_toast(adw::Toast::new(&message));
            });
        });
    }
    {
        let weak = Rc::downgrade(&view);
        let stack = deps.stack.clone();
        let navigation = deps.editor_navigation.clone();
        deps.nav_button.connect_clicked(move |_| {
            let weak = weak.clone();
            let stack = stack.clone();
            navigation.request(move || {
                stack.set_visible_child_name(PAGE_NAME);
                if let Some(view) = weak.upgrade() {
                    view.render();
                }
            });
        });
    }
    {
        let nav_button = deps.nav_button.clone();
        deps.stack.connect_visible_child_name_notify(move |stack| {
            let active = stack.visible_child_name().as_deref() == Some(PAGE_NAME);
            if active {
                nav_button.add_css_class("backups-navigation-active");
            } else {
                nav_button.remove_css_class("backups-navigation-active");
            }
        });
    }

    view.refresh_i18n();
    // The live-translation callback lives as long as the window and owns the view; widget
    // handlers only hold weak references, so there is no cycle.
    BackupsPage {
        refresh_i18n: Rc::new(move || view.refresh_i18n()),
    }
}

#[cfg(test)]
fn descendants(widget: &gtk4::Widget, found: &mut Vec<gtk4::Widget>) {
    found.push(widget.clone());
    let mut child = widget.first_child();
    while let Some(current) = child {
        descendants(&current, found);
        child = current.next_sibling();
    }
}

/// Native GTK check of the Backups page (run from `native_gtk_smoke`). Automatic backups are
/// not installed in tests: the page shows the "no backup yet" state and never touches disk.
#[cfg(test)]
pub(super) fn native_backups_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let window = adw::ApplicationWindow::builder()
        .default_width(1100)
        .default_height(900)
        .build();
    let toast = adw::ToastOverlay::new();
    let stack = gtk4::Stack::new();
    stack.add_named(
        &gtk4::Box::new(Orientation::Vertical, 0),
        Some("entries_view"),
    );
    toast.set_child(Some(&stack));
    window.set_content(Some(&toast));
    let nav_button = gtk4::Button::new();
    let nav_pill = gtk4::Label::new(None);
    let page = build(BackupsDeps {
        window: window.clone(),
        toast: toast.clone(),
        stack: stack.clone(),
        nav_button: nav_button.clone(),
        nav_pill: nav_pill.clone(),
        user_id: Uuid::new_v4(),
        key: Rc::new(RefCell::new(vec![1; 32])),
        editor_navigation: super::editor_navigation::EditorNavigation::new(&window),
        visible: true,
    });
    assert!(nav_button.get_visible(), "shown to administrators");
    nav_button.emit_clicked();
    assert_eq!(stack.visible_child_name().as_deref(), Some(PAGE_NAME));
    assert!(nav_button.has_css_class("backups-navigation-active"));
    assert!(!nav_pill.get_visible(), "no backup yet is not an alarm");

    let mut widgets = Vec::new();
    descendants(
        &stack
            .child_by_name(PAGE_NAME)
            .ok_or("missing backups page")?,
        &mut widgets,
    );
    let labels: Vec<String> = widgets
        .iter()
        .filter_map(|widget| widget.clone().downcast::<gtk4::Label>().ok())
        .map(|label| label.text().to_string())
        .collect();
    for key in [
        "backups-title",
        "backups-status-never",
        "backups-no-success",
        "backups-now",
    ] {
        assert!(labels.contains(&tr(key)), "{key} not shown");
    }
    let switches: Vec<gtk4::Switch> = widgets
        .iter()
        .filter_map(|widget| widget.clone().downcast::<gtk4::Switch>().ok())
        .collect();
    assert_eq!(switches.len(), 2, "daily and on-close switches");
    assert!(
        switches.iter().all(gtk4::Switch::is_active),
        "daily and on close by default"
    );
    switches[0].set_active(false);
    assert!(
        switches[0].is_active(),
        "settings are not saved in tests, the page re-renders the defaults"
    );
    assert_eq!(schedule_from(false, true), BackupSchedule::OnClose);
    assert_eq!(schedule_from(false, false), BackupSchedule::Off);

    // Live retranslation keeps the page consistent.
    assert!(heelonvault_core::i18n::set_language("fr"));
    (page.refresh_i18n)();
    let mut french = Vec::new();
    descendants(
        &stack
            .child_by_name(PAGE_NAME)
            .ok_or("missing backups page")?,
        &mut french,
    );
    assert!(french.iter().any(|widget| {
        widget
            .clone()
            .downcast::<gtk4::Label>()
            .is_ok_and(|label| label.text() == tr("backups-status-never"))
    }));
    assert!(heelonvault_core::i18n::set_language("en"));
    (page.refresh_i18n)();
    window.destroy();
    Ok(())
}

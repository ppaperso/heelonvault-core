use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use heelonvault_core::services::import_service::ImportCsvFailure;

#[derive(Clone)]
pub struct ImportProgressDialog {
    window: gtk4::Window,
    phase_label: gtk4::Label,
    stats_label: gtk4::Label,
    progress_bar: gtk4::ProgressBar,
    completed: Rc<Cell<bool>>,
}

impl ImportProgressDialog {
    pub fn show(parent: &adw::ApplicationWindow, file_name: &str, total_rows: usize) -> Self {
        let window = gtk4::Window::builder()
            .title(heelonvault_core::tr!("profile-import-progress-title").as_str())
            .transient_for(parent)
            .modal(true)
            .default_width(520)
            .default_height(220)
            .build();

        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        root.set_margin_top(24);
        root.set_margin_bottom(24);
        root.set_margin_start(24);
        root.set_margin_end(24);

        let file_label = gtk4::Label::new(Some(
            heelonvault_core::i18n::tr_args(
                "profile-import-progress-file",
                &[("file", heelonvault_core::i18n::I18nArg::Str(file_name))],
            )
            .as_str(),
        ));
        file_label.set_wrap(true);
        file_label.set_xalign(0.0);

        let phase_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("profile-import-progress-preparing").as_str(),
        ));
        phase_label.set_wrap(true);
        phase_label.set_xalign(0.0);
        phase_label.add_css_class("title-4");

        let stats_label = gtk4::Label::new(Some(
            heelonvault_core::i18n::tr_args(
                "profile-import-progress-stats",
                &[
                    ("processed", heelonvault_core::i18n::I18nArg::Num(0)),
                    (
                        "total",
                        heelonvault_core::i18n::I18nArg::Num(total_rows as i64),
                    ),
                    ("imported", heelonvault_core::i18n::I18nArg::Num(0)),
                    ("failed", heelonvault_core::i18n::I18nArg::Num(0)),
                ],
            )
            .as_str(),
        ));
        stats_label.set_wrap(true);
        stats_label.set_xalign(0.0);
        stats_label.add_css_class("dim-label");

        let progress_bar = gtk4::ProgressBar::new();
        progress_bar.set_show_text(true);
        progress_bar.set_fraction(0.0);
        progress_bar.set_text(Some("0%"));

        root.append(&file_label);
        root.append(&phase_label);
        root.append(&progress_bar);
        root.append(&stats_label);
        window.set_child(Some(&root));
        window.present();

        Self {
            window,
            phase_label,
            stats_label,
            progress_bar,
            completed: Rc::new(Cell::new(false)),
        }
    }

    pub fn update(
        &self,
        processed: usize,
        total_rows: usize,
        imported: usize,
        failed: usize,
        current_title: Option<&str>,
    ) {
        let fraction = if total_rows == 0 {
            0.0
        } else {
            processed as f64 / total_rows as f64
        };
        let percent = (fraction * 100.0).round().clamp(0.0, 100.0) as i32;
        self.progress_bar.set_fraction(fraction.min(1.0));
        self.progress_bar.set_text(Some(&format!("{percent}%")));

        let phase = current_title
            .map(|title| {
                heelonvault_core::i18n::tr_args(
                    "profile-import-progress-processing",
                    &[("title", heelonvault_core::i18n::I18nArg::Str(title))],
                )
            })
            .unwrap_or_else(|| {
                heelonvault_core::tr!("profile-import-progress-default").to_string()
            });
        self.phase_label.set_text(&phase);
        self.stats_label.set_text(
            heelonvault_core::i18n::tr_args(
                "profile-import-progress-stats",
                &[
                    (
                        "processed",
                        heelonvault_core::i18n::I18nArg::Num(processed as i64),
                    ),
                    (
                        "total",
                        heelonvault_core::i18n::I18nArg::Num(total_rows as i64),
                    ),
                    (
                        "imported",
                        heelonvault_core::i18n::I18nArg::Num(imported as i64),
                    ),
                    (
                        "failed",
                        heelonvault_core::i18n::I18nArg::Num(failed as i64),
                    ),
                ],
            )
            .as_str(),
        );
    }

    // Phase 5a: called from async import completion path (not yet wired). Owner: ppaadmin | Due: Phase 5b
    #[allow(dead_code)]
    pub fn complete(&self, summary: &str) {
        self.phase_label
            .set_text(heelonvault_core::tr!("profile-import-progress-completed").as_str());
        self.stats_label.set_text(summary);
        self.progress_bar.set_fraction(1.0);
        self.progress_bar.set_text(Some("100%"));
        self.completed.set(true);
    }

    pub fn close(&self) {
        self.window.close();
        self.completed.set(true);
    }

    pub fn completed_flag(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.completed)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show_summary(
        parent: &adw::ApplicationWindow,
        file_name: &str,
        total_rows: usize,
        imported: usize,
        failed: usize,
        duration: Duration,
        failures: &[ImportCsvFailure],
        reject_report_path: Option<&str>,
    ) {
        let window = gtk4::Window::builder()
            .title(heelonvault_core::tr!("profile-import-progress-completed").as_str())
            .transient_for(parent)
            .modal(true)
            .default_width(760)
            .default_height(620)
            .build();

        let header = gtk4::HeaderBar::new();
        header.set_show_title_buttons(true);
        window.set_titlebar(Some(&header));

        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 18);
        root.set_margin_top(24);
        root.set_margin_bottom(24);
        root.set_margin_start(24);
        root.set_margin_end(24);

        let hero = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
        hero.set_halign(gtk4::Align::Start);

        let icon = gtk4::Image::from_icon_name("document-import-symbolic");
        icon.set_pixel_size(44);

        let hero_text = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        let title = gtk4::Label::new(Some(
            heelonvault_core::tr!("profile-import-result-title").as_str(),
        ));
        title.set_xalign(0.0);
        title.add_css_class("title-1");
        title.set_wrap(true);

        let subtitle = gtk4::Label::new(Some(
            heelonvault_core::i18n::tr_args(
                "profile-import-result-subtitle",
                &[("file", heelonvault_core::i18n::I18nArg::Str(file_name))],
            )
            .as_str(),
        ));
        subtitle.set_xalign(0.0);
        subtitle.set_wrap(true);
        subtitle.add_css_class("dim-label");

        let duration_text = Self::format_duration(duration);
        let summary = gtk4::Label::new(Some(
            heelonvault_core::i18n::tr_args(
                "profile-import-result-summary",
                &[
                    (
                        "imported",
                        heelonvault_core::i18n::I18nArg::Num(imported as i64),
                    ),
                    (
                        "failed",
                        heelonvault_core::i18n::I18nArg::Num(failed as i64),
                    ),
                    (
                        "duration",
                        heelonvault_core::i18n::I18nArg::Str(duration_text.as_str()),
                    ),
                ],
            )
            .as_str(),
        ));
        summary.set_xalign(0.0);
        summary.set_wrap(true);
        summary.add_css_class("heading");

        hero_text.append(&title);
        hero_text.append(&subtitle);
        hero_text.append(&summary);
        hero.append(&icon);
        hero.append(&hero_text);

        let stats_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        for (label_key, value, accent_class) in [
            ("profile-import-result-stat-total", total_rows, None),
            (
                "profile-import-result-stat-imported",
                imported,
                Some("status-role-admin"),
            ),
            (
                "profile-import-result-stat-failed",
                failed,
                Some("status-role-user"),
            ),
        ] {
            let card = gtk4::Frame::new(None);
            card.add_css_class("card");
            card.set_hexpand(true);

            let card_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            card_box.set_margin_top(14);
            card_box.set_margin_bottom(14);
            card_box.set_margin_start(16);
            card_box.set_margin_end(16);

            let value_label = gtk4::Label::new(Some(value.to_string().as_str()));
            value_label.set_xalign(0.0);
            value_label.add_css_class("title-2");
            if let Some(class_name) = accent_class {
                value_label.add_css_class(class_name);
            }

            let caption_text = match label_key {
                "profile-import-result-stat-total" => {
                    heelonvault_core::tr!("profile-import-result-stat-total")
                }
                "profile-import-result-stat-imported" => {
                    heelonvault_core::tr!("profile-import-result-stat-imported")
                }
                _ => heelonvault_core::tr!("profile-import-result-stat-failed"),
            };
            let caption = gtk4::Label::new(Some(caption_text.as_str()));
            caption.set_xalign(0.0);
            caption.add_css_class("dim-label");

            card_box.append(&value_label);
            card_box.append(&caption);
            card.set_child(Some(&card_box));
            stats_row.append(&card);
        }

        let details_box = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
        details_box.set_hexpand(true);

        if let Some(path) = reject_report_path {
            let report_label = gtk4::Label::new(Some(
                heelonvault_core::i18n::tr_args(
                    "profile-import-result-reject-report",
                    &[("path", heelonvault_core::i18n::I18nArg::Str(path))],
                )
                .as_str(),
            ));
            report_label.set_xalign(0.0);
            report_label.set_wrap(true);
            report_label.add_css_class("dim-label");
            details_box.append(&report_label);
        }

        let details_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("profile-import-result-details-title").as_str(),
        ));
        details_label.set_xalign(0.0);
        details_label.add_css_class("title-4");

        details_box.append(&details_label);

        if failures.is_empty() {
            let ok_label = gtk4::Label::new(Some(
                heelonvault_core::tr!("profile-import-result-no-failures").as_str(),
            ));
            ok_label.set_xalign(0.0);
            ok_label.set_wrap(true);
            ok_label.add_css_class("dim-label");
            details_box.append(&ok_label);
        } else {
            let scroller = gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .vscrollbar_policy(gtk4::PolicyType::Automatic)
                .min_content_height(180)
                .max_content_height(260)
                .build();

            let list = gtk4::ListBox::new();
            list.add_css_class("boxed-listbox");

            for failure in failures.iter().take(10) {
                let row = gtk4::ListBoxRow::new();
                row.set_selectable(false);
                row.set_activatable(false);

                let row_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                row_box.set_margin_top(10);
                row_box.set_margin_bottom(10);
                row_box.set_margin_start(12);
                row_box.set_margin_end(12);

                let row_title = gtk4::Label::new(Some(
                    heelonvault_core::i18n::tr_args(
                        "profile-import-summary-item",
                        &[
                            (
                                "row",
                                heelonvault_core::i18n::I18nArg::Num(failure.source_row as i64),
                            ),
                            (
                                "title",
                                heelonvault_core::i18n::I18nArg::Str(failure.title.as_str()),
                            ),
                            (
                                "reason",
                                heelonvault_core::i18n::I18nArg::Str(failure.reason.as_str()),
                            ),
                        ],
                    )
                    .as_str(),
                ));
                row_title.set_xalign(0.0);
                row_title.set_wrap(true);
                row_title.add_css_class("heading");

                row_box.append(&row_title);
                row.set_child(Some(&row_box));
                list.append(&row);
            }

            if failures.len() > 10 {
                let more_row = gtk4::ListBoxRow::new();
                more_row.set_selectable(false);
                more_row.set_activatable(false);

                let more_label = gtk4::Label::new(Some(
                    heelonvault_core::i18n::tr_args(
                        "profile-import-summary-more",
                        &[(
                            "count",
                            heelonvault_core::i18n::I18nArg::Num((failures.len() - 10) as i64),
                        )],
                    )
                    .as_str(),
                ));
                more_label.set_xalign(0.0);
                more_label.add_css_class("dim-label");
                more_label.set_margin_top(10);
                more_label.set_margin_bottom(10);
                more_label.set_margin_start(12);
                more_label.set_margin_end(12);
                more_row.set_child(Some(&more_label));
                list.append(&more_row);
            }

            scroller.set_child(Some(&list));
            details_box.append(&scroller);
        }

        let close_button = gtk4::Button::with_label(heelonvault_core::tr!("common-ok").as_str());
        close_button.add_css_class("suggested-action");
        let window_for_close = window.clone();
        close_button.connect_clicked(move |_| {
            window_for_close.close();
        });

        let action_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        action_box.set_halign(gtk4::Align::End);
        action_box.append(&close_button);

        root.append(&hero);
        root.append(&stats_row);
        root.append(&details_box);
        root.append(&action_box);

        window.set_child(Some(&root));
        window.present();
    }

    fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs();
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;

        match (minutes, seconds) {
            (0, 0) => "< 1 s".to_string(),
            (0, secs) => format!("{secs} s"),
            (mins, 0) => format!("{mins} min"),
            (mins, secs) => format!("{mins} min {secs} s"),
        }
    }
}

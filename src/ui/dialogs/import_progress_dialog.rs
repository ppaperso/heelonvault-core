use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone)]
pub struct ImportProgressDialog {
    window: gtk4::Window,
    phase_label: gtk4::Label,
    stats_label: gtk4::Label,
    progress_bar: gtk4::ProgressBar,
    completed: Rc<Cell<bool>>,
}

impl ImportProgressDialog {
    pub fn show(
        parent: &adw::ApplicationWindow,
        file_name: &str,
        total_rows: usize,
    ) -> Self {
        let window = gtk4::Window::builder()
            .title(crate::tr!("profile-import-progress-title").as_str())
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
            crate::i18n::tr_args(
                "profile-import-progress-file",
                &[("file", crate::i18n::I18nArg::Str(file_name))],
            )
            .as_str(),
        ));
        file_label.set_wrap(true);
        file_label.set_xalign(0.0);

        let phase_label = gtk4::Label::new(Some(
            crate::tr!("profile-import-progress-preparing").as_str(),
        ));
        phase_label.set_wrap(true);
        phase_label.set_xalign(0.0);
        phase_label.add_css_class("title-4");

        let stats_label = gtk4::Label::new(Some(
            crate::i18n::tr_args(
                "profile-import-progress-stats",
                &[
                    ("processed", crate::i18n::I18nArg::Num(0)),
                    ("total", crate::i18n::I18nArg::Num(total_rows as i64)),
                    ("imported", crate::i18n::I18nArg::Num(0)),
                    ("failed", crate::i18n::I18nArg::Num(0)),
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
                crate::i18n::tr_args(
                    "profile-import-progress-processing",
                    &[("title", crate::i18n::I18nArg::Str(title))],
                )
            })
            .unwrap_or_else(|| crate::tr!("profile-import-progress-default").to_string());
        self.phase_label.set_text(&phase);
        self.stats_label.set_text(
            crate::i18n::tr_args(
                "profile-import-progress-stats",
                &[
                    ("processed", crate::i18n::I18nArg::Num(processed as i64)),
                    ("total", crate::i18n::I18nArg::Num(total_rows as i64)),
                    ("imported", crate::i18n::I18nArg::Num(imported as i64)),
                    ("failed", crate::i18n::I18nArg::Num(failed as i64)),
                ],
            )
            .as_str(),
        );
    }

    pub fn complete(&self, summary: &str) {
        self.phase_label
            .set_text(crate::tr!("profile-import-progress-completed").as_str());
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
}

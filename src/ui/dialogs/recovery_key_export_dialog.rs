use std::cell::Cell;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use chrono::Local;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use secrecy::{ExposeSecret, SecretString};

use crate::errors::AppError;
use crate::services::backup_service::BackupService;

pub type ExportFuture = Pin<Box<dyn Future<Output = Result<(), AppError>> + 'static>>;
pub type ExportRunner = Rc<dyn Fn(PathBuf, SecretString) -> ExportFuture>;
pub type FeedbackFn = Rc<dyn Fn(&str, &str)>;

pub struct RecoveryKeyExportDialogDeps<TBackup>
where
    TBackup: BackupService + Send + Sync + 'static,
{
    pub parent_window: gtk4::Window,
    pub backup_service: Arc<TBackup>,
    pub cancel_label_key: &'static str,
    pub on_feedback: FeedbackFn,
    pub on_begin_critical: Option<Rc<dyn Fn()>>,
    pub on_end_critical: Option<Rc<dyn Fn()>>,
    pub run_export: ExportRunner,
}

pub struct RecoveryKeyExportDialog;

impl RecoveryKeyExportDialog {
    pub fn show<TBackup>(deps: RecoveryKeyExportDialogDeps<TBackup>)
    where
        TBackup: BackupService + Send + Sync + 'static,
    {
        let chooser = gtk4::FileChooserNative::builder()
            .title(crate::tr!("profile-export-chooser-title").as_str())
            .transient_for(&deps.parent_window)
            .accept_label(crate::tr!("profile-export-accept").as_str())
            .cancel_label(crate::tr!(deps.cancel_label_key).as_str())
            .action(gtk4::FileChooserAction::Save)
            .build();
        chooser.set_current_name("heelonvault_backup.hvb");

        chooser.connect_response(move |dialog, response| {
            if response != gtk4::ResponseType::Accept {
                dialog.destroy();
                return;
            }

            let selected = dialog.file();
            dialog.destroy();
            let Some(file) = selected else {
                (deps.on_feedback)(
                    crate::tr!("profile-export-accept").as_str(),
                    crate::tr!("profile-export-invalid-destination").as_str(),
                );
                return;
            };
            let Some(mut export_path) = file.path() else {
                (deps.on_feedback)(
                    crate::tr!("profile-export-accept").as_str(),
                    crate::tr!("profile-export-invalid-path").as_str(),
                );
                return;
            };
            if export_path.extension().is_none() {
                export_path.set_extension("hvb");
            }

            let recovery = match deps.backup_service.generate_recovery_key() {
                Ok(value) => value,
                Err(_) => {
                    (deps.on_feedback)(
                        crate::tr!("profile-export-accept").as_str(),
                        crate::tr!("profile-export-recovery-key-failed").as_str(),
                    );
                    return;
                }
            };

            let phrase_text = recovery.recovery_phrase.expose_secret().to_string();
            let recovery_words: Vec<String> = phrase_text
                .split_whitespace()
                .map(|word| word.to_string())
                .collect();
            if recovery_words.len() != 24 {
                (deps.on_feedback)(
                    crate::tr!("profile-export-accept").as_str(),
                    crate::tr!("profile-export-recovery-invalid").as_str(),
                );
                return;
            }

            let confirm_dialog = adw::MessageDialog::new(
                Some(&deps.parent_window),
                Some(crate::tr!("profile-export-recovery-dialog-title").as_str()),
                Some(crate::tr!("profile-export-recovery-dialog-body").as_str()),
            );
            confirm_dialog.add_response("cancel", crate::tr!("common-cancel").as_str());
            confirm_dialog.add_response(
                "confirm",
                crate::tr!("profile-export-recovery-confirm").as_str(),
            );
            confirm_dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
            confirm_dialog.set_response_enabled("confirm", false);

            let content_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .spacing(10)
                .margin_top(8)
                .margin_bottom(8)
                .build();

            let helper_label = gtk4::Label::new(Some(
                crate::tr!("profile-export-recovery-dialog-helper").as_str(),
            ));
            helper_label.set_wrap(true);
            helper_label.set_halign(gtk4::Align::Start);
            helper_label.add_css_class("dim-label");

            let words_flow = gtk4::FlowBox::builder()
                .selection_mode(gtk4::SelectionMode::None)
                .max_children_per_line(2)
                .min_children_per_line(1)
                .column_spacing(8)
                .row_spacing(8)
                .build();

            for (index, word) in recovery_words.iter().enumerate() {
                let chip_box = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Horizontal)
                    .spacing(8)
                    .margin_top(6)
                    .margin_bottom(6)
                    .margin_start(8)
                    .margin_end(8)
                    .build();

                let number_label = gtk4::Label::new(Some(format!("{:02}.", index + 1).as_str()));
                number_label.add_css_class("dim-label");

                let separator_label = gtk4::Label::new(Some("|"));
                separator_label.add_css_class("dim-label");

                let word_label = gtk4::Label::new(Some(word.as_str()));
                word_label.add_css_class("monospace");
                word_label.set_selectable(true);
                word_label.set_xalign(0.0);

                chip_box.append(&number_label);
                chip_box.append(&separator_label);
                chip_box.append(&word_label);

                let frame = gtk4::Frame::new(None);
                frame.set_child(Some(&chip_box));
                words_flow.insert(&frame, -1);
            }

            let words_scroller = gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Never)
                .vscrollbar_policy(gtk4::PolicyType::Automatic)
                .min_content_height(220)
                .max_content_height(300)
                .child(&words_flow)
                .build();

            let actions_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .hexpand(true)
                .build();

            let make_action_button = |icon: &str, label: &str| {
                let button = gtk4::Button::new();
                button.add_css_class("flat");
                let inner = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Horizontal)
                    .spacing(6)
                    .build();
                inner.append(&gtk4::Image::from_icon_name(icon));
                inner.append(&gtk4::Label::new(Some(label)));
                button.set_child(Some(&inner));
                button
            };

            let copy_button = make_action_button(
                "edit-copy-symbolic",
                crate::tr!("profile-export-copy").as_str(),
            );
            let print_button = make_action_button(
                "printer-symbolic",
                crate::tr!("profile-export-print").as_str(),
            );
            let save_button = make_action_button(
                "document-save-symbolic",
                crate::tr!("profile-export-save-txt").as_str(),
            );

            actions_box.append(&copy_button);
            actions_box.append(&print_button);
            actions_box.append(&save_button);

            content_box.append(&helper_label);
            content_box.append(&words_scroller);
            content_box.append(&actions_box);
            confirm_dialog.set_extra_child(Some(&content_box));

            let action_done = Rc::new(Cell::new(false));
            let action_done_for_enable = Rc::clone(&action_done);
            let dialog_for_enable = confirm_dialog.clone();
            let enable_confirm = Rc::new(move || {
                if !action_done_for_enable.get() {
                    action_done_for_enable.set(true);
                    dialog_for_enable.set_response_enabled("confirm", true);
                }
            });

            let phrase_for_copy = phrase_text.clone();
            let enable_for_copy = Rc::clone(&enable_confirm);
            let feedback_for_copy = Rc::clone(&deps.on_feedback);
            copy_button.connect_clicked(move |_| {
                let Some(display) = gtk4::gdk::Display::default() else {
                    feedback_for_copy(
                        crate::tr!("profile-export-accept").as_str(),
                        crate::tr!("profile-export-clipboard-unavailable").as_str(),
                    );
                    return;
                };

                let clipboard = display.clipboard();
                clipboard.set_text(phrase_for_copy.as_str());
                let clipboard_for_clear = clipboard.clone();
                glib::timeout_add_seconds_local(60, move || {
                    clipboard_for_clear.set_text("");
                    glib::ControlFlow::Break
                });

                feedback_for_copy(
                    crate::tr!("profile-export-success-title").as_str(),
                    crate::tr!("profile-export-copied").as_str(),
                );
                enable_for_copy();
            });

            let words_for_print = recovery_words.clone();
            let window_for_print = deps.parent_window.clone();
            let enable_for_print = Rc::clone(&enable_confirm);
            let feedback_for_print = Rc::clone(&deps.on_feedback);
            print_button.connect_clicked(move |_| {
                let print_operation = gtk4::PrintOperation::new();
                print_operation.connect_begin_print(|operation, _| {
                    operation.set_n_pages(1);
                });

                let words = words_for_print.clone();
                let header_text = crate::tr!("profile-export-print-header");
                let date_label = crate::tr!("profile-export-print-date");
                print_operation.connect_draw_page(move |_, print_context, _| {
                    let cr = print_context.cairo_context();

                    let mut y = 36.0_f64;
                    cr.select_font_face(
                        "Monospace",
                        gtk4::cairo::FontSlant::Normal,
                        gtk4::cairo::FontWeight::Bold,
                    );
                    cr.set_font_size(16.0);
                    cr.move_to(36.0, y);
                    let _ = cr.show_text(header_text.as_str());

                    y += 24.0;
                    cr.select_font_face(
                        "Monospace",
                        gtk4::cairo::FontSlant::Normal,
                        gtk4::cairo::FontWeight::Normal,
                    );
                    cr.set_font_size(11.0);
                    let printed_at = Local::now().format("%d/%m/%Y %H:%M").to_string();
                    cr.move_to(36.0, y);
                    let _ =
                        cr.show_text(format!("{}: {}", date_label.as_str(), printed_at).as_str());

                    y += 28.0;
                    cr.set_font_size(12.0);
                    for (index, word) in words.iter().enumerate() {
                        cr.move_to(36.0, y);
                        let _ = cr.show_text(format!("{:02}. {}", index + 1, word).as_str());
                        y += 18.0;
                    }
                });

                match print_operation.run(
                    gtk4::PrintOperationAction::PrintDialog,
                    Some(&window_for_print),
                ) {
                    Ok(result) => {
                        if result != gtk4::PrintOperationResult::Cancel {
                            enable_for_print();
                        }
                    }
                    Err(_) => {
                        feedback_for_print(
                            crate::tr!("profile-export-accept").as_str(),
                            crate::tr!("profile-export-print-failed").as_str(),
                        );
                    }
                }
            });

            let words_for_file = recovery_words.clone();
            let window_for_save = deps.parent_window.clone();
            let cancel_key_for_save = deps.cancel_label_key;
            let enable_for_save = Rc::clone(&enable_confirm);
            let feedback_for_save = Rc::clone(&deps.on_feedback);
            save_button.connect_clicked(move |_| {
                let chooser = gtk4::FileChooserNative::builder()
                    .title(crate::tr!("profile-export-save-key-title").as_str())
                    .transient_for(&window_for_save)
                    .accept_label(crate::tr!("profile-export-save-key-accept").as_str())
                    .cancel_label(crate::tr!(cancel_key_for_save).as_str())
                    .action(gtk4::FileChooserAction::Save)
                    .build();
                chooser.set_current_name("heelonvault_recovery_key.txt");

                let words_for_response = words_for_file.clone();
                let enable_for_response = Rc::clone(&enable_for_save);
                let feedback_for_response = Rc::clone(&feedback_for_save);
                chooser.connect_response(move |dialog, response| {
                    if response != gtk4::ResponseType::Accept {
                        dialog.destroy();
                        return;
                    }

                    let selected = dialog.file();
                    dialog.destroy();
                    let Some(file) = selected else {
                        feedback_for_response(
                            crate::tr!("profile-export-accept").as_str(),
                            crate::tr!("profile-export-save-key-invalid-file").as_str(),
                        );
                        return;
                    };

                    let Some(mut txt_path) = file.path() else {
                        feedback_for_response(
                            crate::tr!("profile-export-accept").as_str(),
                            crate::tr!("profile-export-save-key-invalid-path").as_str(),
                        );
                        return;
                    };

                    if txt_path.extension().is_none() {
                        txt_path.set_extension("txt");
                    }

                    let mut content = format!("{}\n", crate::tr!("profile-export-print-header"));
                    content.push_str(
                        format!(
                            "{}: {}\n\n",
                            crate::tr!("profile-export-print-date"),
                            Local::now().format("%d/%m/%Y %H:%M")
                        )
                        .as_str(),
                    );
                    for (index, word) in words_for_response.iter().enumerate() {
                        content.push_str(format!("{:02}. {}\n", index + 1, word).as_str());
                    }

                    match fs::write(txt_path.as_path(), content.as_bytes()) {
                        Ok(()) => {
                            feedback_for_response(
                                crate::tr!("profile-export-success-title").as_str(),
                                crate::tr!("profile-export-save-key-saved").as_str(),
                            );
                            enable_for_response();
                        }
                        Err(_) => {
                            feedback_for_response(
                                crate::tr!("profile-export-accept").as_str(),
                                crate::tr!("profile-export-save-key-failed").as_str(),
                            );
                        }
                    }
                });

                chooser.show();
            });

            let on_feedback_for_confirm = Rc::clone(&deps.on_feedback);
            let run_export_for_confirm = Rc::clone(&deps.run_export);
            let begin_critical_for_confirm = deps.on_begin_critical.clone();
            let end_critical_for_confirm = deps.on_end_critical.clone();
            confirm_dialog.connect_response(None, move |d, response_id| {
                d.close();
                Self::clear_clipboard();
                if response_id != "confirm" {
                    return;
                }

                let backup_path_for_task = export_path.clone();
                let recovery_for_task = recovery.recovery_phrase.clone();
                let on_feedback_for_result = Rc::clone(&on_feedback_for_confirm);
                let end_critical_for_result = end_critical_for_confirm.clone();

                if let Some(begin) = begin_critical_for_confirm.as_ref() {
                    begin();
                }

                let run_export_for_task = Rc::clone(&run_export_for_confirm);
                glib::MainContext::default().spawn_local(async move {
                    let result =
                        (run_export_for_task)(backup_path_for_task, recovery_for_task).await;

                    if let Some(end) = end_critical_for_result.as_ref() {
                        end();
                    }

                    match result {
                        Ok(()) => {
                            on_feedback_for_result(
                                crate::tr!("profile-export-success-title").as_str(),
                                crate::tr!("profile-export-success-body").as_str(),
                            );
                        }
                        Err(AppError::Authorization(_)) => {
                            on_feedback_for_result(
                                crate::tr!("profile-export-accept").as_str(),
                                crate::tr!("profile-export-admin-required-body").as_str(),
                            );
                        }
                        Err(_) => {
                            on_feedback_for_result(
                                crate::tr!("profile-export-accept").as_str(),
                                crate::tr!("profile-export-failed").as_str(),
                            );
                        }
                    }
                });
            });

            confirm_dialog.connect_close_request(|_| {
                Self::clear_clipboard();
                glib::Propagation::Proceed
            });

            confirm_dialog.present();
        });

        chooser.show();
    }

    fn clear_clipboard() {
        if let Some(display) = gtk4::gdk::Display::default() {
            display.clipboard().set_text("");
        }
    }
}

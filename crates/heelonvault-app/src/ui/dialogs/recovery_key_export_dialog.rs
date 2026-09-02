use std::cell::Cell;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use chrono::Local;
use tokio::runtime::Handle;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use secrecy::{ExposeSecret, SecretString};

use heelonvault_core::errors::AppError;
use heelonvault_core::i18n::I18nArg;
use heelonvault_core::services::backup_service::{BackupMetadata, RecoveryKeyBundle};

pub type ExportFuture = Pin<Box<dyn Future<Output = Result<BackupMetadata, AppError>> + 'static>>;
pub type ExportRunner = Arc<dyn Fn(PathBuf, SecretString) -> ExportFuture + Send + Sync>;
pub type FeedbackFn = Rc<dyn Fn(&str, &str)>;

/// Loads the stored recovery verifier for the acting user, if any.
pub type LoadVerifierFuture =
    Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, AppError>> + 'static>>;
pub type LoadVerifierRunner = Arc<dyn Fn() -> LoadVerifierFuture + Send + Sync>;

/// Checks a re-typed phrase against the stored verifier.
pub type VerifyPhraseFuture = Pin<Box<dyn Future<Output = Result<bool, AppError>> + 'static>>;
pub type VerifyPhraseRunner = Arc<dyn Fn(SecretString, Vec<u8>) -> VerifyPhraseFuture + Send + Sync>;

/// Generates and persists a recovery key when the vault carries none.
pub type ProvisionFuture =
    Pin<Box<dyn Future<Output = Result<RecoveryKeyBundle, AppError>> + 'static>>;
pub type ProvisionRecoveryRunner = Arc<dyn Fn() -> ProvisionFuture + Send + Sync>;

pub struct RecoveryKeyExportDialogDeps {
    pub parent_window: gtk4::Window,
    pub cancel_label_key: &'static str,
    pub on_feedback: FeedbackFn,
    pub on_begin_critical: Option<Rc<dyn Fn()>>,
    pub on_end_critical: Option<Rc<dyn Fn()>>,
    pub run_export: ExportRunner,
    pub load_verifier: LoadVerifierRunner,
    pub verify_phrase: VerifyPhraseRunner,
    pub provision_recovery: ProvisionRecoveryRunner,
    pub runtime_handle: Handle,
}

pub struct RecoveryKeyExportDialog;

impl RecoveryKeyExportDialog {
    pub fn show(deps: RecoveryKeyExportDialogDeps) {
        let deps = Rc::new(deps);

        let chooser = gtk4::FileChooserNative::builder()
            .title(heelonvault_core::tr!("profile-export-chooser-title").as_str())
            .transient_for(&deps.parent_window)
            .accept_label(heelonvault_core::tr!("profile-export-accept").as_str())
            .cancel_label(heelonvault_core::tr!(deps.cancel_label_key).as_str())
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
                    heelonvault_core::tr!("profile-export-accept").as_str(),
                    heelonvault_core::tr!("profile-export-invalid-destination").as_str(),
                );
                return;
            };
            let Some(mut export_path) = file.path() else {
                (deps.on_feedback)(
                    heelonvault_core::tr!("profile-export-accept").as_str(),
                    heelonvault_core::tr!("profile-export-invalid-path").as_str(),
                );
                return;
            };
            if export_path.extension().is_none() {
                export_path.set_extension("hvb");
            }

            let deps_for_task = Rc::clone(&deps);
            let runtime = deps_for_task.runtime_handle.clone();
            let on_feedback = deps_for_task.on_feedback.clone();
            let load_verifier = deps_for_task.load_verifier.clone();
            let export_path_for_task = export_path.clone();

            let (sender, receiver) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let result = runtime.block_on(async move {
                    load_verifier().await
                });
                let _ = sender.send(result);
            });

            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(Some(verifier))) => {
                        Self::present_reentry_dialog(deps_for_task, export_path_for_task, verifier);
                    }
                    Ok(Ok(None)) => {
                        Self::present_provision_warning(deps_for_task, export_path_for_task);
                    }
                    Ok(Err(_)) | Err(_) => {
                        on_feedback(
                            heelonvault_core::tr!("profile-export-accept").as_str(),
                            heelonvault_core::tr!("profile-export-verifier-unavailable").as_str(),
                        );
                    }
                }
            });
        });

        chooser.show();
    }

    /// Nominal path: the user must re-type the phrase they stored at first login.
    fn present_reentry_dialog(
        deps: Rc<RecoveryKeyExportDialogDeps>,
        export_path: PathBuf,
        verifier: Vec<u8>,
    ) {
        let dialog = adw::MessageDialog::new(
            Some(&deps.parent_window),
            Some(heelonvault_core::tr!("profile-export-reentry-title").as_str()),
            Some(heelonvault_core::tr!("profile-export-reentry-body").as_str()),
        );
        dialog.add_response("cancel", heelonvault_core::tr!("common-cancel").as_str());
        dialog.add_response(
            "confirm",
            heelonvault_core::tr!("profile-export-reentry-confirm").as_str(),
        );
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        dialog.set_response_enabled("confirm", false);

        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        let helper_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("profile-export-reentry-helper").as_str(),
        ));
        helper_label.set_wrap(true);
        helper_label.set_halign(gtk4::Align::Start);
        helper_label.add_css_class("dim-label");

        let text_view = gtk4::TextView::new();
        text_view.set_wrap_mode(gtk4::WrapMode::Word);
        text_view.set_accepts_tab(false);
        text_view.add_css_class("monospace");
        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_height(110)
            .child(&text_view)
            .build();
        scroller.add_css_class("frame");

        let counter_label = gtk4::Label::new(None);
        counter_label.set_halign(gtk4::Align::Start);
        counter_label.add_css_class("dim-label");

        content_box.append(&helper_label);
        content_box.append(&scroller);
        content_box.append(&counter_label);
        dialog.set_extra_child(Some(&content_box));

        let buffer = text_view.buffer();
        let dialog_for_gate = dialog.clone();
        let counter_for_gate = counter_label.clone();
        buffer.connect_changed(move |buffer| {
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            let count = text.split_whitespace().count();
            counter_for_gate.set_text(
                heelonvault_core::i18n::tr_args(
                    "profile-export-reentry-counter",
                    &[("count", I18nArg::Num(count as i64))],
                )
                .as_str(),
            );
            dialog_for_gate.set_response_enabled("confirm", count == 24);
        });

        let buffer_for_confirm = buffer.clone();
        dialog.connect_response(None, move |d, response_id| {
            if response_id != "confirm" {
                d.close();
                Self::clear_buffer(&buffer_for_confirm);
                return;
            }
            d.close();

            let raw = buffer_for_confirm.text(
                &buffer_for_confirm.start_iter(),
                &buffer_for_confirm.end_iter(),
                false,
            );
            let phrase = Self::normalize_phrase(raw.as_str());
            Self::clear_buffer(&buffer_for_confirm);

            let deps_for_task = Rc::clone(&deps);
            let path_for_task = export_path.clone();
            let verifier_for_task = verifier.clone();
            let phrase_for_export = phrase.clone();
            let runtime = deps_for_task.runtime_handle.clone();
            let on_feedback = deps_for_task.on_feedback.clone();
            let verify_phrase = deps_for_task.verify_phrase.clone();
            let phrase_for_task = phrase;

            let (sender, receiver) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let result = runtime.block_on(async move {
                    verify_phrase(phrase_for_task, verifier_for_task).await
                });
                let _ = sender.send(result);
            });

            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(true)) => {
                        Self::start_export(deps_for_task, path_for_task, phrase_for_export);
                    }
                    Ok(Ok(false)) => {
                        on_feedback(
                            heelonvault_core::tr!("profile-export-accept").as_str(),
                            heelonvault_core::tr!("profile-export-reentry-mismatch").as_str(),
                        );
                    }
                    Ok(Err(_)) | Err(_) => {
                        on_feedback(
                            heelonvault_core::tr!("profile-export-accept").as_str(),
                            heelonvault_core::tr!("profile-export-verifier-unavailable").as_str(),
                        );
                    }
                }
            });
        });

        dialog.present();
    }

    /// Fallback path: the vault carries no verifier. Never silent — the user is warned
    /// before a new key is minted, since any previously kept key becomes void.
    fn present_provision_warning(deps: Rc<RecoveryKeyExportDialogDeps>, export_path: PathBuf) {
        let dialog = adw::MessageDialog::new(
            Some(&deps.parent_window),
            Some(heelonvault_core::tr!("profile-export-provision-title").as_str()),
            Some(heelonvault_core::tr!("profile-export-provision-body").as_str()),
        );
        dialog.add_response("cancel", heelonvault_core::tr!("common-cancel").as_str());
        dialog.add_response(
            "confirm",
            heelonvault_core::tr!("profile-export-provision-confirm").as_str(),
        );
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);

        dialog.connect_response(None, move |d, response_id| {
            d.close();
            if response_id != "confirm" {
                return;
            }

            let deps_for_task = Rc::clone(&deps);
            let path_for_task = export_path.clone();
            let runtime = deps_for_task.runtime_handle.clone();
            let on_feedback = deps_for_task.on_feedback.clone();
            let provision_recovery = deps_for_task.provision_recovery.clone();

            let (sender, receiver) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let result = runtime.block_on(async move {
                    provision_recovery().await
                });
                let _ = sender.send(result);
            });

            glib::MainContext::default().spawn_local(async move {
                match receiver.await {
                    Ok(Ok(bundle)) => {
                        Self::present_new_key_dialog(deps_for_task, path_for_task, bundle);
                    }
                    Ok(Err(_)) | Err(_) => {
                        on_feedback(
                            heelonvault_core::tr!("profile-export-accept").as_str(),
                            heelonvault_core::tr!("profile-export-provision-failed").as_str(),
                        );
                    }
                }
            });
        });

        dialog.present();
    }

    /// Shows a freshly minted key and forces the user to save it before exporting.
    fn present_new_key_dialog(
        deps: Rc<RecoveryKeyExportDialogDeps>,
        export_path: PathBuf,
        bundle: RecoveryKeyBundle,
    ) {
        let phrase_text = bundle.recovery_phrase.expose_secret().to_string();
        let recovery_words: Vec<String> = phrase_text
            .split_whitespace()
            .map(|word| word.to_string())
            .collect();
        if recovery_words.len() != 24 {
            (deps.on_feedback)(
                heelonvault_core::tr!("profile-export-accept").as_str(),
                heelonvault_core::tr!("profile-export-recovery-invalid").as_str(),
            );
            return;
        }

        let dialog = adw::MessageDialog::new(
            Some(&deps.parent_window),
            Some(heelonvault_core::tr!("profile-export-recovery-dialog-title").as_str()),
            Some(heelonvault_core::tr!("profile-export-recovery-dialog-body").as_str()),
        );
        dialog.add_response("cancel", heelonvault_core::tr!("common-cancel").as_str());
        dialog.add_response(
            "confirm",
            heelonvault_core::tr!("profile-export-recovery-confirm").as_str(),
        );
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        dialog.set_response_enabled("confirm", false);

        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        let helper_label = gtk4::Label::new(Some(
            heelonvault_core::tr!("profile-export-recovery-dialog-helper").as_str(),
        ));
        helper_label.set_wrap(true);
        helper_label.set_halign(gtk4::Align::Start);
        helper_label.add_css_class("dim-label");

        let action_done = Rc::new(Cell::new(false));
        let action_done_for_enable = Rc::clone(&action_done);
        let dialog_for_enable = dialog.clone();
        let enable_confirm: Rc<dyn Fn()> = Rc::new(move || {
            if !action_done_for_enable.get() {
                action_done_for_enable.set(true);
                dialog_for_enable.set_response_enabled("confirm", true);
            }
        });

        content_box.append(&helper_label);
        content_box.append(&Self::build_words_view(&recovery_words));
        content_box.append(&Self::build_save_actions(
            &deps,
            &recovery_words,
            phrase_text.as_str(),
            &enable_confirm,
        ));
        dialog.set_extra_child(Some(&content_box));

        let phrase_for_export = bundle.recovery_phrase.clone();
        dialog.connect_response(None, move |d, response_id| {
            d.close();
            Self::clear_clipboard();
            if response_id != "confirm" {
                return;
            }
            Self::start_export(
                Rc::clone(&deps),
                export_path.clone(),
                phrase_for_export.clone(),
            );
        });

        dialog.connect_close_request(|_| {
            Self::clear_clipboard();
            glib::Propagation::Proceed
        });

        dialog.present();
    }

    fn build_words_view(recovery_words: &[String]) -> gtk4::ScrolledWindow {
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

        gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_height(220)
            .max_content_height(300)
            .child(&words_flow)
            .build()
    }

    fn build_save_actions(
        deps: &Rc<RecoveryKeyExportDialogDeps>,
        recovery_words: &[String],
        phrase_text: &str,
        enable_confirm: &Rc<dyn Fn()>,
    ) -> gtk4::Box {
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
            heelonvault_core::tr!("profile-export-copy").as_str(),
        );
        let print_button = make_action_button(
            "printer-symbolic",
            heelonvault_core::tr!("profile-export-print").as_str(),
        );
        let save_button = make_action_button(
            "document-save-symbolic",
            heelonvault_core::tr!("profile-export-save-txt").as_str(),
        );

        let phrase_for_copy = phrase_text.to_string();
        let enable_for_copy = Rc::clone(enable_confirm);
        let feedback_for_copy = Rc::clone(&deps.on_feedback);
        copy_button.connect_clicked(move |_| {
            let Some(display) = gtk4::gdk::Display::default() else {
                feedback_for_copy(
                    heelonvault_core::tr!("profile-export-accept").as_str(),
                    heelonvault_core::tr!("profile-export-clipboard-unavailable").as_str(),
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
                heelonvault_core::tr!("profile-export-success-title").as_str(),
                heelonvault_core::tr!("profile-export-copied").as_str(),
            );
            enable_for_copy();
        });

        let words_for_print = recovery_words.to_vec();
        let window_for_print = deps.parent_window.clone();
        let enable_for_print = Rc::clone(enable_confirm);
        let feedback_for_print = Rc::clone(&deps.on_feedback);
        print_button.connect_clicked(move |_| {
            let print_operation = gtk4::PrintOperation::new();
            print_operation.connect_begin_print(|operation, _| {
                operation.set_n_pages(1);
            });

            let words = words_for_print.clone();
            let header_text = heelonvault_core::tr!("profile-export-print-header");
            let date_label = heelonvault_core::tr!("profile-export-print-date");
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
                let _ = cr.show_text(format!("{}: {}", date_label.as_str(), printed_at).as_str());

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
                        heelonvault_core::tr!("profile-export-accept").as_str(),
                        heelonvault_core::tr!("profile-export-print-failed").as_str(),
                    );
                }
            }
        });

        let words_for_file = recovery_words.to_vec();
        let window_for_save = deps.parent_window.clone();
        let cancel_key_for_save = deps.cancel_label_key;
        let enable_for_save = Rc::clone(enable_confirm);
        let feedback_for_save = Rc::clone(&deps.on_feedback);
        save_button.connect_clicked(move |_| {
            let chooser = gtk4::FileChooserNative::builder()
                .title(heelonvault_core::tr!("profile-export-save-key-title").as_str())
                .transient_for(&window_for_save)
                .accept_label(heelonvault_core::tr!("profile-export-save-key-accept").as_str())
                .cancel_label(heelonvault_core::tr!(cancel_key_for_save).as_str())
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
                        heelonvault_core::tr!("profile-export-accept").as_str(),
                        heelonvault_core::tr!("profile-export-save-key-invalid-file").as_str(),
                    );
                    return;
                };

                let Some(mut txt_path) = file.path() else {
                    feedback_for_response(
                        heelonvault_core::tr!("profile-export-accept").as_str(),
                        heelonvault_core::tr!("profile-export-save-key-invalid-path").as_str(),
                    );
                    return;
                };

                if txt_path.extension().is_none() {
                    txt_path.set_extension("txt");
                }

                let mut content =
                    format!("{}\n", heelonvault_core::tr!("profile-export-print-header"));
                content.push_str(
                    format!(
                        "{}: {}\n\n",
                        heelonvault_core::tr!("profile-export-print-date"),
                        Local::now().format("%d/%m/%Y %H:%M")
                    )
                    .as_str(),
                );
                for (index, word) in words_for_response.iter().enumerate() {
                    content.push_str(format!("{:02}. {}\n", index + 1, word).as_str());
                }

                match Self::write_owner_only(txt_path.as_path(), content.as_bytes()) {
                    Ok(()) => {
                        feedback_for_response(
                            heelonvault_core::tr!("profile-export-success-title").as_str(),
                            heelonvault_core::tr!("profile-export-save-key-saved").as_str(),
                        );
                        enable_for_response();
                    }
                    Err(_) => {
                        feedback_for_response(
                            heelonvault_core::tr!("profile-export-accept").as_str(),
                            heelonvault_core::tr!("profile-export-save-key-failed").as_str(),
                        );
                    }
                }
            });

            chooser.show();
        });

        actions_box.append(&copy_button);
        actions_box.append(&print_button);
        actions_box.append(&save_button);
        actions_box
    }

    fn start_export(
        deps: Rc<RecoveryKeyExportDialogDeps>,
        export_path: PathBuf,
        recovery_phrase: SecretString,
    ) {
        if let Some(begin) = deps.on_begin_critical.as_ref() {
            begin();
        }

        let runtime = deps.runtime_handle.clone();
        let on_feedback = deps.on_feedback.clone();
        let on_end_critical = deps.on_end_critical.clone();
        let run_export = deps.run_export.clone();

        let (sender, receiver) = tokio::sync::oneshot::channel();

        let export_path_for_task = export_path.clone();
        let recovery_phrase_for_task = recovery_phrase.clone();
        std::thread::spawn(move || {
            let result = runtime.block_on(async move {
                run_export(export_path_for_task, recovery_phrase_for_task).await
            });
            let _ = sender.send(result);
        });

        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(result) => {
                    if let Some(end) = on_end_critical.as_ref() {
                        end();
                    }

                    match result {
                        Ok(metadata) => {
                            let display_path = export_path.display().to_string();
                            let body = heelonvault_core::i18n::tr_args(
                                "profile-export-success-detail",
                                &[
                                    ("path", I18nArg::Str(display_path.as_str())),
                                    ("size", I18nArg::Num(metadata.plaintext_size as i64)),
                                ],
                            );
                            on_feedback(
                                heelonvault_core::tr!("profile-export-success-title").as_str(),
                                body.as_str(),
                            );
                        }
                        Err(AppError::Authorization(_)) => {
                            on_feedback(
                                heelonvault_core::tr!("profile-export-accept").as_str(),
                                heelonvault_core::tr!("profile-export-admin-required-body").as_str(),
                            );
                        }
                        Err(_) => {
                            on_feedback(
                                heelonvault_core::tr!("profile-export-accept").as_str(),
                                heelonvault_core::tr!("profile-export-failed").as_str(),
                            );
                        }
                    }
                }
                Err(_) => {
                    if let Some(end) = on_end_critical.as_ref() {
                        end();
                    }
                    on_feedback(
                        heelonvault_core::tr!("profile-export-accept").as_str(),
                        heelonvault_core::tr!("profile-export-failed").as_str(),
                    );
                }
            }
        });
    }

    fn normalize_phrase(raw: &str) -> SecretString {
        let normalized = raw
            .split_whitespace()
            .map(str::to_lowercase)
            .collect::<Vec<_>>()
            .join(" ");
        SecretString::new(normalized.into_boxed_str())
    }

    fn clear_buffer(buffer: &gtk4::TextBuffer) {
        buffer.set_text("");
    }

    fn write_owner_only(path: &std::path::Path, content: &[u8]) -> std::io::Result<()> {
        fs::write(path, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    fn clear_clipboard() {
        if let Some(display) = gtk4::gdk::Display::default() {
            display.clipboard().set_text("");
        }
    }
}

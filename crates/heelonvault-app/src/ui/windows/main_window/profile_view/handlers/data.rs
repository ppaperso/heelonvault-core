//! Export and import handlers for the data section.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use tokio::runtime::Handle;
use uuid::Uuid;

use heelonvault_core::services::backup_application_service::BackupApplicationService;
use heelonvault_core::services::backup_service::BackupService;
use heelonvault_core::services::import_service::ImportService;
use heelonvault_core::services::secret_service::SecretService;
use heelonvault_core::services::vault_service::VaultService;

use secrecy::{ExposeSecret, SecretBox};

use heelonvault_core::repositories::user_repository::{SqlxUserRepository, UserRepository};
use heelonvault_core::services::crypto_service::{CryptoService, CryptoServiceImpl};

use super::super::sections::data::DataSection;
use crate::ui::dialogs::recovery_key_export_dialog;
use crate::ui::dialogs::recovery_key_export_dialog::{
    ExportRunner, RecoveryKeyExportDialog, RecoveryKeyExportDialogDeps,
};
use crate::ui::windows::main_window::MainWindow;

pub struct DataHandlerDeps<TBackup, TBackupApp, TImport, TSecret, TVault> {
    pub window: adw::ApplicationWindow,
    pub runtime_handle: Handle,
    pub backup_service: Arc<TBackup>,
    pub backup_app_service: Arc<TBackupApp>,
    pub import_service: Arc<TImport>,
    pub secret_service: Arc<TSecret>,
    pub vault_service: Arc<TVault>,
    pub user_repo: Arc<SqlxUserRepository>,
    pub crypto_service: Arc<CryptoServiceImpl>,
    pub database_path: PathBuf,
    pub user_id: Uuid,
    pub is_admin: bool,
    pub session_master_key: Rc<RefCell<Vec<u8>>>,
    pub on_import_completed_refresh: Rc<dyn Fn()>,
    pub begin_critical_operation: Rc<dyn Fn()>,
    pub end_critical_operation: Rc<dyn Fn()>,
}

/// Wire the export (.hvb backup) and import (CSV) buttons.
pub fn setup<TBackup, TBackupApp, TImport, TSecret, TVault>(
    data_section: &DataSection,
    deps: DataHandlerDeps<TBackup, TBackupApp, TImport, TSecret, TVault>,
) where
    TBackup: BackupService + Send + Sync + 'static,
    TBackupApp: BackupApplicationService + Send + Sync + 'static,
    TImport: ImportService + Send + Sync + 'static,
    TSecret: SecretService + Send + Sync + 'static,
    TVault: VaultService + Send + Sync + 'static,
{
    let DataHandlerDeps {
        window,
        runtime_handle,
        backup_service,
        backup_app_service,
        import_service,
        secret_service,
        vault_service,
        user_repo,
        crypto_service,
        database_path,
        user_id,
        is_admin,
        session_master_key,
        on_import_completed_refresh,
        begin_critical_operation,
        end_critical_operation,
    } = deps;

    let window_for_export = window.clone();
    let backup_app_for_export: Arc<TBackupApp> = Arc::clone(&backup_app_service);
    let backup_for_export = Arc::clone(&backup_service);
    let database_path_for_export = database_path.clone();
    let actor_user_id_for_export: Uuid = user_id;
    let is_admin_for_export = is_admin;
    let begin_critical_for_export = Rc::clone(&begin_critical_operation);
    let end_critical_for_export = Rc::clone(&end_critical_operation);
    let session_for_export = Rc::clone(&session_master_key);
    let runtime_for_export = runtime_handle.clone();
    data_section.export_button.connect_clicked(move |_| {
        if is_admin_for_export {
            let window_for_dialog = window_for_export.clone();
            let backup_for_dialog = Arc::clone(&backup_for_export);
            let backup_app_for_dialog = Arc::clone(&backup_app_for_export);
            let db_path_for_dialog = database_path_for_export.clone();
            let actor_id_for_dialog = actor_user_id_for_export;
            let begin_critical_for_dialog = Rc::clone(&begin_critical_for_export);
            let end_critical_for_dialog = Rc::clone(&end_critical_for_export);

            let window_for_feedback = window_for_dialog.clone();
            let on_feedback: Rc<dyn Fn(&str, &str)> = Rc::new(move |title, body| {
                MainWindow::show_feedback_dialog(&window_for_feedback, title, body);
            });

            let run_export: ExportRunner =
                Arc::new(move |backup_path: PathBuf, recovery_phrase| {
                    let backup_app_for_task = Arc::clone(&backup_app_for_dialog);
                    let db_for_task = db_path_for_dialog.clone();
                    Box::pin(async move {
                        backup_app_for_task
                            .export_backup_secured(
                                actor_id_for_dialog,
                                db_for_task.as_path(),
                                backup_path.as_path(),
                                &recovery_phrase,
                            )
                            .await
                    })
                });

            let user_repo_for_verifier = Arc::clone(&user_repo);
            let user_id_for_verifier = actor_user_id_for_export;
            let load_verifier: recovery_key_export_dialog::LoadVerifierRunner =
                Arc::new(move || {
                    let user_repo = Arc::clone(&user_repo_for_verifier);
                    Box::pin(
                        async move { user_repo.get_recovery_verifier(user_id_for_verifier).await },
                    )
                });

            let backup_for_verify = Arc::clone(&backup_for_dialog);
            let verify_phrase: recovery_key_export_dialog::VerifyPhraseRunner =
                Arc::new(move |phrase, verifier| {
                    let backup_service = Arc::clone(&backup_for_verify);
                    Box::pin(async move {
                        backup_service.verify_recovery_phrase(&phrase, verifier.as_slice())
                    })
                });

            let user_repo_for_provision = Arc::clone(&user_repo);
            let crypto_for_provision = Arc::clone(&crypto_service);
            let backup_for_provision = Arc::clone(&backup_for_dialog);
            let master_key_for_provision =
                MainWindow::snapshot_session_master_key(&session_for_export);
            let provision_recovery: recovery_key_export_dialog::ProvisionRecoveryRunner =
                Arc::new(move || {
                    let user_repo = Arc::clone(&user_repo_for_provision);
                    let crypto = Arc::clone(&crypto_for_provision);
                    let backup_service = Arc::clone(&backup_for_provision);
                    let master_key_bytes = master_key_for_provision.clone();

                    Box::pin(async move {
                        let Some(master_key_bytes) = master_key_bytes else {
                            return Err(heelonvault_core::errors::AppError::Validation(
                                "session master key unavailable".to_string(),
                            ));
                        };
                        let master_key = SecretBox::new(Box::new(master_key_bytes));

                        let bundle = backup_service.generate_recovery_key()?;

                        let encrypted = crypto
                            .encrypt(
                                &SecretBox::new(Box::new(
                                    bundle.recovery_phrase.expose_secret().as_bytes().to_vec(),
                                )),
                                &master_key,
                            )
                            .await?;
                        user_repo
                            .set_recovery_phrase_envelope(
                                user_id_for_verifier,
                                heelonvault_core::services::crypto_service::encode_envelope(
                                    &encrypted,
                                ),
                            )
                            .await?;

                        let verifier =
                            backup_service.build_recovery_verifier(&bundle.recovery_phrase)?;
                        user_repo
                            .set_recovery_verifier(user_id_for_verifier, verifier)
                            .await?;

                        Ok(bundle)
                    })
                });

            RecoveryKeyExportDialog::show(RecoveryKeyExportDialogDeps {
                parent_window: window_for_dialog.upcast::<gtk4::Window>(),
                cancel_label_key: "trash-dialog-cancel",
                on_feedback,
                on_begin_critical: Some(begin_critical_for_dialog),
                on_end_critical: Some(end_critical_for_dialog),
                run_export,
                load_verifier,
                verify_phrase,
                provision_recovery,
                runtime_handle: runtime_for_export.clone(),
            });
        } else {
            MainWindow::show_feedback_dialog(
                &window_for_export,
                heelonvault_core::tr!("profile-export-admin-required-title").as_str(),
                heelonvault_core::tr!("profile-export-admin-required-body").as_str(),
            );
        }
    });

    let window_for_import = window.clone();
    let import_for_profile = Arc::clone(&import_service);
    let secret_for_import = Arc::clone(&secret_service);
    let vault_for_import = Arc::clone(&vault_service);
    let runtime_for_import = runtime_handle.clone();
    let session_for_import = Rc::clone(&session_master_key);
    let refresh_for_import = Rc::clone(&on_import_completed_refresh);
    let begin_critical_for_import = Rc::clone(&begin_critical_operation);
    let end_critical_for_import = Rc::clone(&end_critical_operation);
    data_section.import_button.connect_clicked(move |_| {
        let chooser = gtk4::FileChooserNative::builder()
            .title(heelonvault_core::tr!("profile-import-chooser-title").as_str())
            .transient_for(&window_for_import)
            .accept_label(heelonvault_core::tr!("profile-import-accept").as_str())
            .cancel_label(heelonvault_core::tr!("trash-dialog-cancel").as_str())
            .action(gtk4::FileChooserAction::Open)
            .build();

        let window_for_response = window_for_import.clone();
        let import_for_response = Arc::clone(&import_for_profile);
        let secret_for_response = Arc::clone(&secret_for_import);
        let vault_for_response = Arc::clone(&vault_for_import);
        let runtime_for_response = runtime_for_import.clone();
        let session_for_response = Rc::clone(&session_for_import);
        let refresh_for_response = Rc::clone(&refresh_for_import);
        let begin_critical_for_response = Rc::clone(&begin_critical_for_import);
        let end_critical_for_response = Rc::clone(&end_critical_for_import);

        chooser.connect_response(move |dialog, response| {
            if response != gtk4::ResponseType::Accept {
                dialog.destroy();
                return;
            }

            let selected = dialog.file();
            dialog.destroy();
            let Some(file) = selected else {
                MainWindow::show_feedback_dialog(
                    &window_for_response,
                    heelonvault_core::tr!("profile-import-accept").as_str(),
                    heelonvault_core::tr!("profile-import-invalid-file").as_str(),
                );
                return;
            };
            let Some(csv_path) = file.path() else {
                MainWindow::show_feedback_dialog(
                    &window_for_response,
                    heelonvault_core::tr!("profile-import-accept").as_str(),
                    heelonvault_core::tr!("profile-import-invalid-path").as_str(),
                );
                return;
            };

            let Some(master_key) = MainWindow::snapshot_session_master_key(&session_for_response)
            else {
                MainWindow::show_feedback_dialog(
                    &window_for_response,
                    heelonvault_core::tr!("profile-import-accept").as_str(),
                    heelonvault_core::tr!("profile-import-session-locked").as_str(),
                );
                return;
            };

            let preview = match import_for_response.preview_csv(csv_path.as_path()) {
                Ok(preview) => preview,
                Err(error) => {
                    tracing::error!(error = %error, csv_file = %csv_path.display(), "csv import preview failed");
                    MainWindow::show_feedback_dialog(
                        &window_for_response,
                        heelonvault_core::tr!("profile-import-accept").as_str(),
                        format!("Cannot read CSV preview:\n{}", error).as_str(),
                    );
                    return;
                }
            };

            // Fetch accessible vaults to let user choose the import target.
            let available_vaults: Vec<heelonvault_core::models::Vault> = {
                let vault_svc = Arc::clone(&vault_for_response);
                let rt = runtime_for_response.clone();
                match std::thread::spawn(move || {
                    rt.block_on(vault_svc.list_user_vaults(user_id))
                })
                .join()
                {
                    Ok(Ok(v)) => v,
                    _ => vec![],
                }
            };

            let vault_combo = gtk4::ComboBoxText::new();
            for vault in &available_vaults {
                vault_combo.append(Some(&vault.id.to_string()), &vault.name);
            }
            if !available_vaults.is_empty() {
                vault_combo.set_active(Some(0));
            }
            let vault_selector_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
            vault_selector_box.set_margin_top(12);
            let vault_label = gtk4::Label::new(Some(
                heelonvault_core::tr!("profile-import-vault-label").as_str(),
            ));
            vault_label.set_xalign(0.0);
            vault_selector_box.append(&vault_label);
            vault_selector_box.append(&vault_combo);

            let file_name = csv_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("CSV file")
                .to_string();

            let preview_body = format!(
                "{}\n{}\n{}\n\n{}",
                heelonvault_core::i18n::tr_args(
                    "profile-import-preview-file",
                    &[("file", heelonvault_core::i18n::I18nArg::Str(file_name.as_str()))],
                ),
                heelonvault_core::i18n::tr_args(
                    "profile-import-preview-detected",
                    &[("count", heelonvault_core::i18n::I18nArg::Num(preview.total_rows as i64))],
                ),
                heelonvault_core::i18n::tr_args(
                    "profile-import-preview-ready",
                    &[("count", heelonvault_core::i18n::I18nArg::Num(preview.importable_rows as i64))],
                ),
                heelonvault_core::tr!("profile-import-preview-confirm"),
            );

            let preview_dialog = adw::MessageDialog::new(
                Some(&window_for_response),
                Some(heelonvault_core::tr!("profile-import-preview-title").as_str()),
                Some(preview_body.as_str()),
            );
            preview_dialog.add_response("cancel", heelonvault_core::tr!("common-cancel").as_str());
            preview_dialog.add_response("import", heelonvault_core::tr!("profile-import-accept").as_str());
            preview_dialog.set_default_response(Some("import"));
            preview_dialog.set_close_response("cancel");
            preview_dialog.set_extra_child(Some(&vault_selector_box));

            let window_for_preview = window_for_response.clone();
            let import_for_preview = Arc::clone(&import_for_response);
            let secret_for_preview = Arc::clone(&secret_for_response);
            let vault_for_preview = Arc::clone(&vault_for_response);
            let runtime_for_preview = runtime_for_response.clone();
            let refresh_for_preview = Rc::clone(&refresh_for_response);
            let begin_critical_for_preview = Rc::clone(&begin_critical_for_response);
            let end_critical_for_preview = Rc::clone(&end_critical_for_response);
            let vault_combo_for_preview = vault_combo.clone();

            preview_dialog.connect_response(None, move |preview_dialog, preview_response| {
                preview_dialog.close();
                if preview_response != "import" {
                    return;
                }

                let target_vault_id: Option<uuid::Uuid> = vault_combo_for_preview
                    .active_id()
                    .and_then(|id| uuid::Uuid::parse_str(&id).ok());

                let progress_dialog = Rc::new(
                    crate::ui::dialogs::import_progress_dialog::ImportProgressDialog::show(
                        &window_for_preview,
                        file_name.as_str(),
                        preview.total_rows,
                    ),
                );
                let progress_done = progress_dialog.completed_flag();
                let import_started_at = std::time::Instant::now();
                let (progress_tx, progress_rx) = std::sync::mpsc::channel::<
                    heelonvault_core::services::import_service::ImportProgressEvent,
                >();

                let progress_dialog_for_timer = Rc::clone(&progress_dialog);
                glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
                    while let Ok(event) = progress_rx.try_recv() {
                        match event {
                            heelonvault_core::services::import_service::ImportProgressEvent::Started {
                                total_rows,
                                importable_rows,
                                failed_rows,
                            } => {
                                progress_dialog_for_timer.update(
                                    failed_rows,
                                    total_rows,
                                    0,
                                    failed_rows,
                                    Some(&format!(
                                        "Pre-flight: {} ready, {} to review",
                                        importable_rows, failed_rows
                                    )),
                                );
                            }
                            heelonvault_core::services::import_service::ImportProgressEvent::Progress {
                                processed,
                                total_rows,
                                imported,
                                failed,
                                current_title,
                            } => {
                                progress_dialog_for_timer.update(
                                    processed,
                                    total_rows,
                                    imported,
                                    failed,
                                    current_title.as_deref(),
                                );
                            }
                        }
                    }

                    if progress_done.get() {
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                });

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let import_for_task = Arc::clone(&import_for_preview);
                let secret_for_task = Arc::clone(&secret_for_preview);
                let vault_for_task = Arc::clone(&vault_for_preview);
                let runtime_for_task = runtime_for_preview.clone();
                let csv_path_for_task = csv_path.clone();
                let master_key_for_task = master_key.clone();
                let file_name_for_result = file_name.clone();
                begin_critical_for_preview();
                std::thread::spawn(move || {
                    let result = runtime_for_task.block_on(async move {
                        import_for_task
                            .import_csv(
                                csv_path_for_task.as_path(),
                                user_id,
                                SecretBox::new(Box::new(master_key_for_task)),
                                target_vault_id,
                                secret_for_task,
                                vault_for_task,
                                Some(progress_tx),
                            )
                            .await
                    });
                    let _ = sender.send(result);
                });

                let window_for_result = window_for_preview.clone();
                let refresh_for_result = Rc::clone(&refresh_for_preview);
                let end_critical_for_result = Rc::clone(&end_critical_for_preview);
                let progress_dialog_for_result = Rc::clone(&progress_dialog);
                glib::MainContext::default().spawn_local(async move {
                    let result = receiver.await;
                    end_critical_for_result();
                    match result {
                        Ok(Ok(report)) => {
                            refresh_for_result();
                            progress_dialog_for_result.close();
                            crate::ui::dialogs::import_progress_dialog::ImportProgressDialog::show_summary(
                                &window_for_result,
                                file_name_for_result.as_str(),
                                report.total_rows,
                                report.imported,
                                report.failed,
                                import_started_at.elapsed(),
                                &report.failures,
                                report.reject_report_path.as_deref(),
                            );
                        }
                        Ok(Err(error)) => {
                            progress_dialog_for_result.close();
                            tracing::error!(
                                error = %error,
                                "csv import failed in profile view"
                            );
                            MainWindow::show_feedback_dialog(
                                &window_for_result,
                                heelonvault_core::tr!("profile-import-accept").as_str(),
                                heelonvault_core::i18n::tr_args(
                                    "profile-import-error-body",
                                    &[("error", heelonvault_core::i18n::I18nArg::Str(error.to_string().as_str()))],
                                )
                                .as_str(),
                            );
                        }
                        Err(error) => {
                            progress_dialog_for_result.close();
                            tracing::error!(
                                error = %error,
                                "csv import worker channel failed"
                            );
                            MainWindow::show_feedback_dialog(
                                &window_for_result,
                                heelonvault_core::tr!("profile-import-accept").as_str(),
                                heelonvault_core::tr!("profile-import-failed").as_str(),
                            );
                        }
                    }
                });
            });

            preview_dialog.present();
        });

        chooser.show();
    });
}

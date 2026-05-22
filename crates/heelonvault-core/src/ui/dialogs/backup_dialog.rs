#![allow(clippy::type_complexity)]

use std::rc::Rc;
use std::sync::Arc;

use gtk4::prelude::*;

use crate::services::audit_service::{AuditAction, AuditService};
use crate::services::backup_service::BackupService;

pub struct BackupDialogDeps<TBackup>
where
    TBackup: BackupService + Send + Sync + 'static,
{
    pub parent_window: gtk4::Window,
    pub backup_service: Arc<TBackup>,
    pub audit_service: Arc<AuditService>,
    pub on_feedback: Rc<dyn Fn(&str, &str)>,
    pub on_begin_critical: Option<Rc<dyn Fn()>>,
    pub on_end_critical: Option<Rc<dyn Fn()>>,
}

pub struct BackupDialog;

impl BackupDialog {
    pub fn show<TBackup>(deps: BackupDialogDeps<TBackup>)
    where
        TBackup: BackupService + Send + Sync + 'static,
    {
        // Log backup action
        deps.audit_service.log_async(
            None,
            AuditAction::SecretView, // Using SecretView as placeholder
            Some("backup"),
            None,
            Some("Backup operation initiated"),
        );

        // Create file chooser dialog for selecting backup destination
        let chooser = gtk4::FileChooserNative::builder()
            .title(crate::tr!("backup-dialog-title").as_str())
            .transient_for(&deps.parent_window)
            .accept_label(crate::tr!("backup-dialog-accept").as_str())
            .cancel_label(crate::tr!("common-cancel").as_str())
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
                    crate::tr!("backup-dialog-accept").as_str(),
                    crate::tr!("backup-dialog-invalid-destination").as_str(),
                );
                return;
            };

            let Some(_backup_path) = file.path() else {
                (deps.on_feedback)(
                    crate::tr!("backup-dialog-accept").as_str(),
                    crate::tr!("backup-dialog-invalid-path").as_str(),
                );
                return;
            };

            // Attempt to generate recovery key
            match deps.backup_service.generate_recovery_key() {
                Ok(_key_bundle) => {
                    // Log successful backup key generation
                    deps.audit_service.log_async(
                        None,
                        AuditAction::SecretView, // Using SecretView as placeholder
                        Some("backup"),
                        None,
                        Some("Backup recovery key generated successfully"),
                    );
                    (deps.on_feedback)(
                        crate::tr!("backup-dialog-success-title").as_str(),
                        crate::tr!("backup-dialog-success-message").as_str(),
                    );
                }
                Err(e) => {
                    // Log failed backup
                    deps.audit_service.log_async(
                        None,
                        AuditAction::AuthLoginFailure, // Using LoginFailure as placeholder
                        Some("backup"),
                        None,
                        Some(&format!("Backup failed: {}", e)),
                    );
                    (deps.on_feedback)(
                        crate::tr!("backup-dialog-error-title").as_str(),
                        crate::tr!("backup-dialog-error-message").as_str(),
                    );
                }
            }
        });

        chooser.show();
    }
}

//! Automatic backup triggers bound to the session (see `crate::ui::auto_backup`).

use heelonvault_core::services::auto_backup::BackupTrigger;
use libadwaita as adw;
use libadwaita::prelude::*;

use super::MainWindow;
use crate::ui::auto_backup;

impl MainWindow {
    /// Starts a backup for `trigger` when due, with the current session key. Unlock, manual
    /// and password-change runs report their result as a toast; a close-time run is silent
    /// (the window is going away) and is reported at the next unlock if it failed.
    pub fn request_auto_backup(&self, trigger: BackupTrigger) {
        let overlay = self
            .window
            .content()
            .and_downcast::<adw::ToastOverlay>()
            .filter(|_| trigger != BackupTrigger::Close);
        let key = self.session_master_key.borrow();
        auto_backup::request(self.session_user_id, &key, trigger, move |record| {
            let Some(overlay) = overlay else {
                return;
            };
            let message = match record {
                Some(record) => Some(auto_backup::toast_message(&record)),
                None if trigger == BackupTrigger::Unlock => {
                    auto_backup::unlock_reminder(&auto_backup::journal())
                }
                None => None,
            };
            if let Some(message) = message {
                overlay.add_toast(adw::Toast::new(&message));
            }
        });
    }
}

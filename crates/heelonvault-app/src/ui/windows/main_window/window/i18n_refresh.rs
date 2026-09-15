//! Live re-translation of the main window.
//!
//! Changing the language from the profile page must not require a restart, so every static
//! label built at startup is re-applied from the freshly loaded locale.

use std::rc::Rc;

use gtk4::prelude::*;

use crate::ui::windows::main_window::sidebar;
use crate::ui::windows::main_window::{CenterPanelWidgets, SidebarWidgets};

/// Header and stack widgets that carry translatable text.
pub struct I18nTargets {
    pub sidebar_panel: SidebarWidgets,
    pub center_panel: CenterPanelWidgets,
    pub shell_refresh: Rc<dyn Fn()>,
    pub profile_button: gtk4::MenuButton,
    pub profile_title: gtk4::Label,
    pub add_button: gtk4::Button,
    pub trash_button: gtk4::Button,
    pub panic_button: gtk4::Button,
    pub panic_label: gtk4::Label,
    pub profile_container: gtk4::ScrolledWindow,
    pub editor_host: gtk4::Box,
}

/// Build the callback that re-applies every translation of the main window.
pub fn build_refresh(
    targets: I18nTargets,
    #[cfg(feature = "premium")] admin_pages: Option<super::navigation::AdminPages>,
) -> Rc<dyn Fn()> {
    Rc::new(move || {
        sidebar::refresh_i18n(&targets.sidebar_panel);
        (targets.shell_refresh)();

        targets.profile_button.set_tooltip_text(Some(
            heelonvault_core::tr!("main-last-logins-tooltip").as_str(),
        ));
        targets
            .profile_title
            .set_text(heelonvault_core::tr!("main-last-logins-title").as_str());
        targets
            .add_button
            .set_tooltip_text(Some(heelonvault_core::tr!("main-add-tooltip").as_str()));
        targets
            .trash_button
            .set_tooltip_text(Some(heelonvault_core::tr!("main-trash-tooltip").as_str()));
        targets
            .panic_button
            .set_tooltip_text(Some(heelonvault_core::tr!("main-panic-tooltip").as_str()));
        targets
            .panic_label
            .set_text(heelonvault_core::tr!("main-panic-label").as_str());

        let center = &targets.center_panel;
        center.status_total_chip.set_tooltip_text(Some(
            heelonvault_core::tr!("main-status-total-tooltip").as_str(),
        ));
        center.status_non_compliant_chip.set_tooltip_text(Some(
            heelonvault_core::tr!("main-status-noncompliant-tooltip").as_str(),
        ));
        center.status_incomplete_chip.set_tooltip_text(Some(
            heelonvault_core::tr!("main-status-incomplete-tooltip").as_str(),
        ));
        center.status_never_used_chip.set_tooltip_text(Some(
            heelonvault_core::tr!("main-status-never-used-tooltip").as_str(),
        ));
        center.sort_recent_button.set_tooltip_text(Some(
            heelonvault_core::tr!("main-sort-recent-tooltip").as_str(),
        ));
        center.sort_title_button.set_tooltip_text(Some(
            heelonvault_core::tr!("main-sort-title-tooltip").as_str(),
        ));
        center.sort_risk_button.set_tooltip_text(Some(
            heelonvault_core::tr!("main-sort-risk-tooltip").as_str(),
        ));
        center
            .filtered_status_page
            .set_title(heelonvault_core::tr!("main-filtered-empty-title").as_str());
        center.filtered_status_page.set_description(Some(
            heelonvault_core::tr!("main-filtered-empty-description").as_str(),
        ));

        center
            .stack
            .page(&center.list_page)
            .set_title(heelonvault_core::tr!("main-stack-grid").as_str());
        center
            .stack
            .page(&center.empty_state)
            .set_title(heelonvault_core::tr!("main-stack-empty").as_str());

        center
            .main_stack
            .page(&center.stack)
            .set_title(heelonvault_core::tr!("main-stack-secrets").as_str());
        center
            .main_stack
            .page(&targets.profile_container)
            .set_title(heelonvault_core::tr!("main-profile-security").as_str());
        center
            .main_stack
            .page(&targets.editor_host)
            .set_title(heelonvault_core::tr!("main-stack-editor").as_str());

        #[cfg(feature = "premium")]
        if let Some(pages) = admin_pages.as_ref() {
            pages.refresh_i18n(&center.main_stack);
        }
    })
}

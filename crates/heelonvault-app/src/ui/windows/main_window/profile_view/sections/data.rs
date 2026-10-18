//! Encrypted backup export and CSV import, one row each.

use gtk4::Align;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone)]
pub struct DataSection {
    pub frame: adw::PreferencesGroup,
    pub export_button: gtk4::Button,
    pub import_button: gtk4::Button,
    export_row: adw::ActionRow,
    import_row: adw::ActionRow,
}

fn action_row(icon_name: &str) -> (adw::ActionRow, gtk4::Button) {
    let row = adw::ActionRow::new();
    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.add_css_class("profile-row-icon");
    row.add_prefix(&icon);
    let button = gtk4::Button::new();
    button.add_css_class("flat");
    button.add_css_class("profile-row-button");
    button.set_valign(Align::Center);
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    (row, button)
}

/// `can_export`: the encrypted export covers the whole database and is reserved to admins;
/// the row is not shown to other users rather than failing after a click.
pub fn build(can_export: bool) -> DataSection {
    let frame = adw::PreferencesGroup::new();
    frame.add_css_class("profile-group");
    let (export_row, export_button) = action_row("hv-save-symbolic");
    let (import_row, import_button) = action_row("hv-document-symbolic");
    export_row.set_visible(can_export);
    frame.add(&export_row);
    frame.add(&import_row);
    let section = DataSection {
        frame,
        export_button,
        import_button,
        export_row,
        import_row,
    };
    section.refresh_i18n();
    section
}

impl DataSection {
    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-data").as_str());
        self.export_row
            .set_title(heelonvault_core::tr!("profile-export-title").as_str());
        self.export_row
            .set_subtitle(heelonvault_core::tr!("profile-export-subtitle").as_str());
        self.export_button
            .set_label(heelonvault_core::tr!("profile-export-button").as_str());
        self.import_row
            .set_title(heelonvault_core::tr!("profile-import-title").as_str());
        self.import_row
            .set_subtitle(heelonvault_core::tr!("profile-import-subtitle").as_str());
        self.import_button
            .set_label(heelonvault_core::tr!("profile-import-button").as_str());
    }
}

//! Backup export and CSV import entry points.

use gtk4::Align;
use gtk4::Orientation;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone)]
pub struct DataSection {
    pub frame: adw::PreferencesGroup,
    pub export_button: gtk4::Button,
    pub import_button: gtk4::Button,
    export_title: gtk4::Label,
    export_subtitle: gtk4::Label,
    import_title: gtk4::Label,
    import_subtitle: gtk4::Label,
}

pub fn build() -> DataSection {
    let frame = adw::PreferencesGroup::new();
    frame.set_title(heelonvault_core::tr!("profile-section-data").as_str());
    frame.add_css_class("profile-section-frame");
    frame.set_hexpand(true);

    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let export_title = section_title("profile-export-title");
    let export_subtitle = section_subtitle("profile-export-subtitle");
    let export_button = action_button("profile-export-button");

    let import_title = section_title("profile-import-title");
    let import_subtitle = section_subtitle("profile-import-subtitle");
    let import_button = action_button("profile-import-button");

    for widget in [
        export_title.upcast_ref::<gtk4::Widget>(),
        export_subtitle.upcast_ref::<gtk4::Widget>(),
        export_button.upcast_ref::<gtk4::Widget>(),
        import_title.upcast_ref::<gtk4::Widget>(),
        import_subtitle.upcast_ref::<gtk4::Widget>(),
        import_button.upcast_ref::<gtk4::Widget>(),
    ] {
        content.append(widget);
    }
    frame.add(&content);

    DataSection {
        frame,
        export_button,
        import_button,
        export_title,
        export_subtitle,
        import_title,
        import_subtitle,
    }
}

fn section_title(key: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(heelonvault_core::i18n::tr(key).as_str()));
    label.set_halign(Align::Start);
    label.add_css_class("heading");
    label.add_css_class("profile-field-label");
    label
}

fn section_subtitle(key: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(heelonvault_core::i18n::tr(key).as_str()));
    label.set_halign(Align::Start);
    label.add_css_class("dim-label");
    label.add_css_class("profile-section-subtitle");
    label.set_wrap(true);
    label
}

fn action_button(key: &str) -> gtk4::Button {
    let button = gtk4::Button::with_label(heelonvault_core::i18n::tr(key).as_str());
    button.add_css_class("suggested-action");
    button.add_css_class("profile-action-btn");
    button.set_halign(Align::End);
    button
}

impl DataSection {
    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-data").as_str());
        self.export_title
            .set_text(heelonvault_core::tr!("profile-export-title").as_str());
        self.export_subtitle
            .set_text(heelonvault_core::tr!("profile-export-subtitle").as_str());
        self.export_button
            .set_label(heelonvault_core::tr!("profile-export-button").as_str());
        self.import_title
            .set_text(heelonvault_core::tr!("profile-import-title").as_str());
        self.import_subtitle
            .set_text(heelonvault_core::tr!("profile-import-subtitle").as_str());
        self.import_button
            .set_label(heelonvault_core::tr!("profile-import-button").as_str());
    }
}

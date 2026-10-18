//! Interface preferences. The language applies as soon as it is chosen: it is a preference,
//! not part of the account form.

use libadwaita as adw;
use libadwaita::prelude::*;

/// Language codes, in the order of the combo row.
pub const LANGUAGES: [&str; 2] = ["fr", "en"];
const LANGUAGE_KEYS: [&str; 2] = ["language-option-fr", "language-option-en"];

#[derive(Clone)]
pub struct PreferencesSection {
    pub frame: adw::PreferencesGroup,
    pub language_items: gtk4::StringList,
    pub language_row: adw::ComboRow,
}

fn translated_languages() -> Vec<String> {
    LANGUAGE_KEYS
        .iter()
        .map(|key| heelonvault_core::i18n::tr(key))
        .collect()
}

pub fn build() -> PreferencesSection {
    let frame = adw::PreferencesGroup::new();
    frame.add_css_class("profile-group");
    let labels = translated_languages();
    let language_items =
        gtk4::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>());
    let language_row = adw::ComboRow::new();
    language_row.set_model(Some(&language_items));
    let icon = gtk4::Image::from_icon_name("hv-globe-symbolic");
    icon.add_css_class("profile-row-icon");
    language_row.add_prefix(&icon);
    frame.add(&language_row);
    let section = PreferencesSection {
        frame,
        language_items,
        language_row,
    };
    section.refresh_i18n();
    section
}

impl PreferencesSection {
    pub fn selected_language(&self) -> &'static str {
        LANGUAGES
            .get(self.language_row.selected() as usize)
            .copied()
            .unwrap_or(LANGUAGES[0])
    }

    pub fn select_language(&self, code: &str) {
        let index = LANGUAGES
            .iter()
            .position(|language| code.starts_with(language))
            .unwrap_or(0);
        self.language_row.set_selected(index as u32);
    }

    pub fn refresh_i18n(&self) {
        self.frame
            .set_title(heelonvault_core::tr!("profile-section-preferences").as_str());
        self.language_row
            .set_title(heelonvault_core::tr!("profile-language-title").as_str());
        self.language_row
            .set_subtitle(heelonvault_core::tr!("profile-language-subtitle").as_str());
        let selected = self.language_row.selected();
        let labels = translated_languages();
        self.language_items.splice(
            0,
            self.language_items.n_items(),
            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        self.language_row.set_selected(selected);
    }
}

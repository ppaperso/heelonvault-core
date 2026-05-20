use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::Orientation;

/// Widgets and i18n refresh for the main content shell (search bar + paned view).
pub(super) struct ContentShell {
    pub(super) container: gtk4::Box,
    pub(super) search_entry: gtk4::SearchEntry,
    pub(super) multivault_toggle: gtk4::ToggleButton,
    /// Refreshes all i18n strings owned by the shell (search entry, toggle, help labels).
    pub(super) refresh_i18n: Rc<dyn Fn()>,
}

pub(super) fn build_content_shell(
    sidebar_frame: &gtk4::Frame,
    center_frame: &gtk4::Frame,
) -> ContentShell {
    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(14)
        .margin_bottom(14)
        .margin_start(14)
        .margin_end(14)
        .build();

    let actions_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .build();

    // ── MultiVault toggle ─────────────────────────────────────────────────
    let multivault_toggle = gtk4::ToggleButton::builder()
        .label(crate::tr!("main-search-multivault-label").as_str())
        .tooltip_text(crate::tr!("main-search-multivault-tooltip").as_str())
        .valign(gtk4::Align::Center)
        .build();
    multivault_toggle.add_css_class("flat");
    multivault_toggle.add_css_class("pill");
    actions_row.append(&multivault_toggle);
    // ─────────────────────────────────────────────────────────────────────

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text(crate::tr!("main-search-placeholder").as_str())
        .hexpand(true)
        .build();
    search_entry.add_css_class("main-search-entry");
    actions_row.append(&search_entry);

    // ── Search help button (premium popover) ─────────────────────────────
    let help_popover = gtk4::Popover::new();
    help_popover.set_position(gtk4::PositionType::Bottom);
    help_popover.set_has_arrow(true);
    help_popover.set_autohide(true);

    // Outer container — fixed width for consistent layout
    let help_content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .width_request(348)
        .build();

    // ── Header row: icon + title ──────────────────────────────────────────
    let header_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .margin_bottom(10)
        .build();
    let header_icon = gtk4::Image::from_icon_name("edit-find-symbolic");
    header_icon.add_css_class("dim-label");
    let help_title_lbl = gtk4::Label::builder()
        .label(crate::tr!("main-search-help-title").as_str())
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .build();
    help_title_lbl.add_css_class("heading");
    header_row.append(&header_icon);
    header_row.append(&help_title_lbl);
    help_content.append(&header_row);
    help_content.append(&gtk4::Separator::new(Orientation::Horizontal));

    // ── Body sections ─────────────────────────────────────────────────────
    let body_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_top(10)
        .build();

    // Section 1: no-prefix search
    let no_prefix_title_lbl = gtk4::Label::builder()
        .label(crate::tr!("main-search-help-no-prefix-title").as_str())
        .halign(gtk4::Align::Start)
        .build();
    no_prefix_title_lbl.add_css_class("caption-heading");
    let no_prefix_body_lbl = gtk4::Label::builder()
        .label(crate::tr!("main-search-help-no-prefix-body").as_str())
        .halign(gtk4::Align::Start)
        .wrap(true)
        .build();
    no_prefix_body_lbl.add_css_class("caption");
    no_prefix_body_lbl.add_css_class("dim-label");
    body_box.append(&no_prefix_title_lbl);
    body_box.append(&no_prefix_body_lbl);

    body_box.append(&gtk4::Separator::new(Orientation::Horizontal));

    // Section 2: field:value prefix
    let prefix_title_lbl = gtk4::Label::builder()
        .label(crate::tr!("main-search-help-prefix-title").as_str())
        .halign(gtk4::Align::Start)
        .build();
    prefix_title_lbl.add_css_class("caption-heading");
    let fields_lbl = gtk4::Label::builder()
        .label(crate::tr!("main-search-help-fields").as_str())
        .halign(gtk4::Align::Start)
        .wrap(true)
        .build();
    fields_lbl.add_css_class("caption");
    fields_lbl.add_css_class("monospace");
    let examples_lbl = gtk4::Label::builder()
        .label(crate::tr!("main-search-help-examples").as_str())
        .halign(gtk4::Align::Start)
        .wrap(true)
        .margin_top(2)
        .build();
    examples_lbl.add_css_class("caption");
    examples_lbl.add_css_class("monospace");
    examples_lbl.add_css_class("dim-label");
    body_box.append(&prefix_title_lbl);
    body_box.append(&fields_lbl);
    body_box.append(&examples_lbl);

    body_box.append(&gtk4::Separator::new(Orientation::Horizontal));

    // Section 3: fuzzy note
    let fuzzy_lbl = gtk4::Label::builder()
        .label(crate::tr!("main-search-help-fuzzy").as_str())
        .halign(gtk4::Align::Start)
        .wrap(true)
        .build();
    fuzzy_lbl.add_css_class("caption");
    fuzzy_lbl.add_css_class("dim-label");
    body_box.append(&fuzzy_lbl);

    help_content.append(&body_box);
    help_popover.set_child(Some(&help_content));

    let help_button = gtk4::MenuButton::builder()
        .icon_name("help-browser-symbolic")
        .tooltip_text(crate::tr!("main-search-help-tooltip").as_str())
        .popover(&help_popover)
        .valign(gtk4::Align::Center)
        .build();
    help_button.add_css_class("flat");
    help_button.add_css_class("circular");
    actions_row.append(&help_button);
    // ─────────────────────────────────────────────────────────────────────

    let split = gtk4::Paned::builder()
        .orientation(Orientation::Horizontal)
        .wide_handle(true)
        .vexpand(true)
        .build();
    split.set_position(270);
    split.set_start_child(Some(sidebar_frame));
    split.set_end_child(Some(center_frame));

    content.append(&actions_row);
    content.append(&split);

    // ── i18n refresh closure — updates all shell-owned labels ─────────────
    let refresh_i18n: Rc<dyn Fn()> = {
        let multivault_toggle_r = multivault_toggle.clone();
        let search_entry_r = search_entry.clone();
        let help_button_r = help_button.clone();
        let help_title_lbl_r = help_title_lbl.clone();
        let no_prefix_title_lbl_r = no_prefix_title_lbl.clone();
        let no_prefix_body_lbl_r = no_prefix_body_lbl.clone();
        let prefix_title_lbl_r = prefix_title_lbl.clone();
        let fields_lbl_r = fields_lbl.clone();
        let examples_lbl_r = examples_lbl.clone();
        let fuzzy_lbl_r = fuzzy_lbl.clone();
        Rc::new(move || {
            multivault_toggle_r.set_label(crate::tr!("main-search-multivault-label").as_str());
            multivault_toggle_r
                .set_tooltip_text(Some(crate::tr!("main-search-multivault-tooltip").as_str()));
            search_entry_r
                .set_placeholder_text(Some(crate::tr!("main-search-placeholder").as_str()));
            help_button_r.set_tooltip_text(Some(crate::tr!("main-search-help-tooltip").as_str()));
            help_title_lbl_r.set_text(crate::tr!("main-search-help-title").as_str());
            no_prefix_title_lbl_r.set_text(crate::tr!("main-search-help-no-prefix-title").as_str());
            no_prefix_body_lbl_r.set_text(crate::tr!("main-search-help-no-prefix-body").as_str());
            prefix_title_lbl_r.set_text(crate::tr!("main-search-help-prefix-title").as_str());
            fields_lbl_r.set_text(crate::tr!("main-search-help-fields").as_str());
            examples_lbl_r.set_text(crate::tr!("main-search-help-examples").as_str());
            fuzzy_lbl_r.set_text(crate::tr!("main-search-help-fuzzy").as_str());
        })
    };

    ContentShell {
        container: content,
        search_entry,
        multivault_toggle,
        refresh_i18n,
    }
}

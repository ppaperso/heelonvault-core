use gtk4::prelude::*;
use gtk4::Orientation;

pub(super) fn build_content_shell(
    sidebar_frame: &gtk4::Frame,
    center_frame: &gtk4::Frame,
) -> (gtk4::Box, gtk4::SearchEntry) {
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
        .spacing(12)
        .build();

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text(crate::tr!("main-search-placeholder").as_str())
        .hexpand(true)
        .build();
    search_entry.add_css_class("main-search-entry");
    actions_row.append(&search_entry);

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

    (content, search_entry)
}

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;

use super::CenterPanelWidgets;

pub(super) fn build_center_panel() -> CenterPanelWidgets {
    let center_frame = gtk4::Frame::new(None);
    center_frame.add_css_class("main-center-panel");

    let entries_stack = gtk4::Stack::builder()
        .vexpand(true)
        .hexpand(true)
        .transition_type(gtk4::StackTransitionType::Crossfade)
        .build();

    let list_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .hexpand(true)
        .build();
    list_scroll.add_css_class("main-secret-grid-scroll");

    let secret_flow = gtk4::FlowBox::builder()
        .homogeneous(true)
        .max_children_per_line(5)
        .min_children_per_line(1)
        .row_spacing(16)
        .column_spacing(16)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .selection_mode(gtk4::SelectionMode::None)
        .halign(gtk4::Align::Start)
        .valign(gtk4::Align::Start)
        .build();
    secret_flow.add_css_class("main-secret-grid");
    list_scroll.set_child(Some(&secret_flow));

    let filtered_status_page = adw::StatusPage::builder()
        .title(crate::tr!("main-filtered-empty-title").as_str())
        .description(crate::tr!("main-filtered-empty-description").as_str())
        .icon_name("edit-find-symbolic")
        .build();
    filtered_status_page.set_visible(false);
    filtered_status_page.set_can_target(false);

    let list_overlay = gtk4::Overlay::new();
    list_overlay.set_child(Some(&list_scroll));
    list_overlay.add_overlay(&filtered_status_page);

    let status_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(0)
        .margin_start(12)
        .margin_end(12)
        .build();
    status_row.add_css_class("vault-secret-status-row");

    let metrics_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .hexpand(true)
        .build();

    let (status_total_chip, status_total_badge) =
        build_status_metric_chip("view-grid-symbolic", "0", false);
    let (status_non_compliant_chip, status_non_compliant_badge) =
        build_status_metric_chip("dialog-warning-symbolic", "0", true);
    metrics_box.append(&status_total_chip);
    metrics_box.append(&status_non_compliant_chip);

    let sort_switch = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(4)
        .halign(Align::End)
        .build();
    sort_switch.add_css_class("vault-secret-sort-switch");

    let sort_recent_button = build_status_sort_button("view-sort-descending-symbolic");
    let sort_title_button = build_status_sort_button("insert-text-symbolic");
    let sort_risk_button = build_status_sort_button("dialog-warning-symbolic");
    sort_switch.append(&sort_recent_button);
    sort_switch.append(&sort_title_button);
    sort_switch.append(&sort_risk_button);

    status_row.append(&metrics_box);
    status_row.append(&sort_switch);

    let list_page = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .vexpand(true)
        .hexpand(true)
        .build();
    list_page.append(&status_row);
    list_page.append(&list_overlay);

    let empty_state = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .halign(Align::Center)
        .valign(Align::Center)
        .vexpand(true)
        .hexpand(true)
        .build();
    empty_state.add_css_class("main-empty-state");

    let empty_icon = gtk4::Image::from_resource(
        "/com/heelonvault/rust/icons/hicolor/128x128/apps/heelonvault.png",
    );
    empty_icon.set_pixel_size(64);
    empty_icon.add_css_class("main-empty-icon");

    let empty_title = gtk4::Label::new(Some(crate::tr!("main-empty-title").as_str()));
    empty_title.add_css_class("title-3");
    empty_title.add_css_class("main-empty-title");

    let empty_description = gtk4::Label::new(Some(crate::tr!("main-empty-description").as_str()));
    empty_description.set_wrap(true);
    empty_description.set_justify(gtk4::Justification::Center);
    empty_description.set_max_width_chars(54);
    empty_description.add_css_class("main-empty-copy");

    empty_state.append(&empty_icon);
    empty_state.append(&empty_title);
    empty_state.append(&empty_description);

    entries_stack.add_titled(
        &list_page,
        Some("list"),
        crate::tr!("main-stack-grid").as_str(),
    );
    entries_stack.add_titled(
        &empty_state,
        Some("empty"),
        crate::tr!("main-stack-empty").as_str(),
    );
    entries_stack.set_visible_child_name("empty");

    let main_stack = gtk4::Stack::builder()
        .vexpand(true)
        .hexpand(true)
        .transition_type(gtk4::StackTransitionType::Crossfade)
        .build();
    main_stack.set_transition_duration(200);
    main_stack.add_titled(
        &entries_stack,
        Some("entries_view"),
        crate::tr!("main-stack-secrets").as_str(),
    );
    main_stack.set_visible_child_name("entries_view");

    center_frame.set_child(Some(&main_stack));
    CenterPanelWidgets {
        frame: center_frame,
        main_stack,
        stack: entries_stack,
        list_page,
        empty_state,
        secret_flow,
        filtered_status_page,
        status_total_chip,
        status_total_badge,
        status_non_compliant_chip,
        status_non_compliant_badge,
        sort_recent_button,
        sort_title_button,
        sort_risk_button,
        empty_title,
        empty_copy: empty_description,
    }
}

fn build_status_metric_chip(
    icon_name: &str,
    count: &str,
    warning: bool,
) -> (gtk4::Box, gtk4::Label) {
    let chip = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();
    chip.add_css_class("vault-secret-status-chip");
    if warning {
        chip.add_css_class("vault-secret-status-chip-warning");
    }

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_pixel_size(16);
    icon.add_css_class("vault-secret-status-icon");
    chip.append(&icon);

    let badge = gtk4::Label::new(Some(count));
    badge.add_css_class("audit-count-badge");
    badge.add_css_class("vault-secret-status-badge");
    if warning {
        badge.add_css_class("vault-secret-status-badge-warning");
    }
    chip.append(&badge);

    (chip, badge)
}

fn build_status_sort_button(icon_name: &str) -> gtk4::Button {
    let button = gtk4::Button::builder().icon_name(icon_name).build();
    button.add_css_class("flat");
    button.add_css_class("vault-secret-sort-button");
    button
}

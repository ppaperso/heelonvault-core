//! Status chips above the secret list.
//!
//! One row, hidden when there is nothing to report: a load pill only when the list is not a
//! plain complete load (loading, partial, unavailable, multi-vault scope), a retry button when
//! useful, and one removable chip per active filter with a "visible/loaded" counter. The full
//! explanatory sentences stay available as tooltips instead of permanent text.

use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use heelonvault_core::i18n::{I18nArg, tr_args};

use super::list_load::ListLoadState;
use super::{AuditFilter, SecretCategoryFilter};

/// Window action clearing one list filter: `type`, `audit`, `search` or `all`.
/// Registered as `clear-list-filter` on the main window (`window/core.rs`).
pub(super) const CLEAR_FILTER_ACTION: &str = "win.clear-list-filter";

/// Builds the chip area: `(area, load label, retry button, filter chips)`. Two compact
/// lines, each shown only when needed: the load pill (whose first child is its icon) with the
/// retry button, then the active filter chips, which get the full width.
pub(super) fn build_row() -> (gtk4::Box, gtk4::Label, gtk4::Button, gtk4::Box) {
    let area = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .build();
    area.add_css_class(AREA_CLASS);
    area.set_visible(false);

    let status_line = gtk4::Box::new(Orientation::Horizontal, 8);
    status_line.add_css_class(LINE_CLASS);
    status_line.set_visible(false);

    let pill = gtk4::Box::new(Orientation::Horizontal, 6);
    pill.add_css_class("main-list-pill");
    pill.set_valign(Align::Center);
    let icon = gtk4::Image::from_icon_name("hv-info-symbolic");
    icon.set_pixel_size(14);
    let label = gtk4::Label::builder()
        .xalign(0.0)
        .single_line_mode(true)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    pill.append(&icon);
    pill.append(&label);
    pill.set_visible(false);

    let retry = gtk4::Button::with_label(heelonvault_core::tr!("main-list-retry").as_str());
    retry.add_css_class("main-list-retry");
    retry.set_valign(Align::Center);
    retry.set_visible(false);
    status_line.append(&pill);
    status_line.append(&retry);

    let chips = gtk4::Box::new(Orientation::Horizontal, 6);
    chips.add_css_class("main-filter-chips");
    chips.add_css_class(LINE_CLASS);
    chips.set_visible(false);

    area.append(&status_line);
    area.append(&chips);
    (area, label, retry, chips)
}

const AREA_CLASS: &str = "main-list-chips";
const LINE_CLASS: &str = "main-list-chips-line";

fn any_child_visible(widget: &gtk4::Widget) -> bool {
    let mut child = widget.first_child();
    while let Some(current) = child {
        // Own `visible` flag: `is_visible()` would also require the (hidden) parents.
        if current.get_visible() {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

/// Shows each line, and the area, only when one of their parts has something to say.
fn sync_row(member: &impl IsA<gtk4::Widget>) {
    let mut widget = member.as_ref().clone();
    if !widget.has_css_class(LINE_CLASS) {
        let Some(line) = widget.parent() else {
            return;
        };
        line.set_visible(any_child_visible(&line));
        widget = line;
    }
    if let Some(area) = widget.parent()
        && area.has_css_class(AREA_CLASS)
    {
        area.set_visible(any_child_visible(&area));
    }
}

/// Short pill content for a load state: `(icon, text, warning)`; `None` for a plain complete
/// single-vault load, which needs no message.
pub(super) fn load_pill(
    state: ListLoadState,
    no_selection: bool,
) -> Option<(&'static str, String, bool)> {
    match state {
        ListLoadState::Idle | ListLoadState::Locked => None,
        ListLoadState::Loading => Some((
            "hv-clock-symbolic",
            heelonvault_core::tr!("main-list-pill-loading"),
            false,
        )),
        ListLoadState::Failed => Some((
            "hv-warning-symbolic",
            heelonvault_core::tr!("main-list-pill-failed"),
            true,
        )),
        ListLoadState::Ready { .. } if no_selection => Some((
            "hv-vault-symbolic",
            heelonvault_core::tr!("main-no-vault-selected-title"),
            false,
        )),
        ListLoadState::Ready { coverage, global } => {
            if coverage.partial() {
                let mut text = tr_args(
                    "main-list-pill-partial",
                    &[
                        ("loaded", I18nArg::Num(coverage.loaded_vaults as i64)),
                        ("total", I18nArg::Num(coverage.vaults as i64)),
                    ],
                );
                if coverage.failed_items > 0 {
                    text.push_str(" · ");
                    text.push_str(&tr_args(
                        "main-list-pill-unreadable",
                        &[("items", I18nArg::Num(coverage.failed_items as i64))],
                    ));
                }
                Some(("hv-warning-symbolic", text, true))
            } else if global {
                Some((
                    "hv-vault-symbolic",
                    tr_args(
                        "main-list-pill-global",
                        &[("total", I18nArg::Num(coverage.vaults as i64))],
                    ),
                    false,
                ))
            } else {
                None
            }
        }
    }
}

/// Renders the load pill and the retry button; the full sentence becomes the tooltip.
pub(super) fn render_load(
    label: &gtk4::Label,
    retry: &gtk4::Button,
    state: ListLoadState,
    no_selection: bool,
) {
    let pill = label.parent();
    match load_pill(state, no_selection) {
        Some((icon, text, warning)) => {
            label.set_text(&text);
            let tooltip = if no_selection {
                text.clone()
            } else {
                state.message()
            };
            if let Some(pill) = pill.as_ref() {
                pill.set_tooltip_text(Some(&tooltip));
                if let Some(image) = pill.first_child().and_downcast::<gtk4::Image>() {
                    image.set_icon_name(Some(icon));
                }
                if warning {
                    pill.add_css_class("main-list-pill-warning");
                } else {
                    pill.remove_css_class("main-list-pill-warning");
                }
                pill.set_visible(true);
            }
            if warning {
                label.add_css_class("warning");
            } else {
                label.remove_css_class("warning");
            }
        }
        None => {
            label.set_text("");
            label.remove_css_class("warning");
            if let Some(pill) = pill.as_ref() {
                pill.set_visible(false);
                pill.set_tooltip_text(None);
            }
        }
    }
    retry.set_label(heelonvault_core::tr!("main-list-retry").as_str());
    let show_retry = matches!(state, ListLoadState::Failed)
        || matches!(state, ListLoadState::Ready { coverage, .. } if coverage.partial());
    retry.set_visible(show_retry);
    sync_row(retry);
}

/// Filters currently applied to the list, with the resulting counts.
pub(super) struct ActiveFilters<'a> {
    pub category: SecretCategoryFilter,
    pub audit: AuditFilter,
    pub search: &'a str,
    pub visible: usize,
    pub loaded: usize,
}

impl ActiveFilters<'_> {
    pub(super) fn count(&self) -> usize {
        usize::from(self.category != SecretCategoryFilter::All)
            + usize::from(self.audit != AuditFilter::All)
            + usize::from(!self.search.trim().is_empty())
    }
}

fn category_chip(category: SecretCategoryFilter) -> Option<(&'static str, String)> {
    let (icon, key) = match category {
        SecretCategoryFilter::All => return None,
        SecretCategoryFilter::Password => ("hv-key-symbolic", "main-category-passwords"),
        SecretCategoryFilter::Passphrase => ("hv-passphrase-symbolic", "main-category-passphrases"),
        SecretCategoryFilter::ApiToken => ("hv-code-symbolic", "main-category-api-tokens"),
        SecretCategoryFilter::SshKey => ("hv-terminal-symbolic", "main-category-ssh-keys"),
        SecretCategoryFilter::SecureDocument => ("hv-document-symbolic", "main-category-documents"),
    };
    Some((icon, heelonvault_core::i18n::tr(key)))
}

fn audit_chip(audit: AuditFilter) -> Option<(&'static str, String)> {
    match audit {
        AuditFilter::All => None,
        AuditFilter::Weak => Some((
            "hv-warning-symbolic",
            heelonvault_core::tr!("main-audit-weak"),
        )),
        AuditFilter::Duplicate => Some((
            "hv-reuse-symbolic",
            heelonvault_core::tr!("main-audit-duplicates"),
        )),
    }
}

/// One removable filter chip: icon, label, cross; a click clears that filter.
fn filter_chip(icon: &str, text: &str, target: &str) -> gtk4::Button {
    let content = gtk4::Box::new(Orientation::Horizontal, 6);
    let image = gtk4::Image::from_icon_name(icon);
    image.set_pixel_size(14);
    let label = gtk4::Label::new(Some(text));
    // Only very long values are shortened; the chips line has the full width.
    label.set_single_line_mode(true);
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_max_width_chars(32);
    let cross = gtk4::Label::new(Some("×"));
    cross.add_css_class("main-filter-chip-cross");
    content.append(&image);
    content.append(&label);
    content.append(&cross);
    let button = gtk4::Button::new();
    button.set_child(Some(&content));
    button.add_css_class("main-filter-chip");
    button.set_valign(Align::Center);
    button.set_tooltip_text(Some(&heelonvault_core::tr!("main-filter-chip-remove")));
    button.set_action_name(Some(CLEAR_FILTER_ACTION));
    button.set_action_target_value(Some(&target.to_variant()));
    button
}

/// Hides the filter chips (list reloading or cleared).
pub(super) fn hide_filters(chips: &gtk4::Box) {
    chips.set_visible(false);
    sync_row(chips);
}

/// Rebuilds the filter chips; the box is hidden when no filter is active.
pub(super) fn render_filters(chips: &gtk4::Box, filters: &ActiveFilters<'_>) {
    while let Some(child) = chips.first_child() {
        chips.remove(&child);
    }
    let active = filters.count();
    if active == 0 {
        chips.set_visible(false);
        sync_row(chips);
        return;
    }
    let count = gtk4::Label::new(Some(&tr_args(
        "main-filter-count",
        &[
            ("visible", I18nArg::Num(filters.visible as i64)),
            ("loaded", I18nArg::Num(filters.loaded as i64)),
        ],
    )));
    count.add_css_class("main-filter-count");
    count.set_valign(Align::Center);
    count.set_tooltip_text(Some(&tr_args(
        "main-filter-count-tooltip",
        &[
            ("visible", I18nArg::Num(filters.visible as i64)),
            ("loaded", I18nArg::Num(filters.loaded as i64)),
        ],
    )));
    chips.append(&count);
    if let Some((icon, text)) = category_chip(filters.category) {
        chips.append(&filter_chip(icon, &text, "type"));
    }
    if let Some((icon, text)) = audit_chip(filters.audit) {
        chips.append(&filter_chip(icon, &text, "audit"));
    }
    let search = filters.search.trim();
    if !search.is_empty() {
        let text = tr_args(
            "main-filter-chip-search",
            &[("query", I18nArg::Str(search))],
        );
        chips.append(&filter_chip("hv-search-symbolic", &text, "search"));
    }
    if active > 1 {
        let clear = gtk4::Button::with_label(&heelonvault_core::tr!("main-filter-clear-all"));
        clear.add_css_class("flat");
        clear.add_css_class("main-filter-clear");
        clear.set_valign(Align::Center);
        clear.set_action_name(Some(CLEAR_FILTER_ACTION));
        clear.set_action_target_value(Some(&"all".to_variant()));
        chips.append(&clear);
    }
    chips.set_visible(true);
    sync_row(chips);
}

#[cfg(test)]
mod tests {
    use super::super::list_load::Coverage;
    use super::*;

    #[test]
    fn plain_complete_load_needs_no_pill() {
        let complete = ListLoadState::Ready {
            coverage: Coverage {
                vaults: 1,
                loaded_vaults: 1,
                failed_vaults: 0,
                failed_items: 0,
            },
            global: false,
        };
        assert!(load_pill(complete, false).is_none());
        assert!(load_pill(ListLoadState::Idle, false).is_none());
        assert!(load_pill(ListLoadState::Locked, false).is_none());
        assert!(load_pill(complete, true).is_some(), "no vault selected");
        let global = ListLoadState::Ready {
            coverage: Coverage {
                vaults: 3,
                loaded_vaults: 3,
                failed_vaults: 0,
                failed_items: 0,
            },
            global: true,
        };
        assert!(matches!(load_pill(global, false), Some((_, _, false))));
    }

    #[test]
    fn partial_and_failed_loads_are_warnings() -> Result<(), &'static str> {
        let partial = ListLoadState::Ready {
            coverage: Coverage {
                vaults: 2,
                loaded_vaults: 1,
                failed_vaults: 1,
                failed_items: 3,
            },
            global: true,
        };
        let (_, text, warning) =
            load_pill(partial, false).ok_or("partial load must show a pill")?;
        assert!(warning);
        let plain = text.replace(['\u{2068}', '\u{2069}'], "");
        assert!(plain.contains("1/2"), "{plain}");
        assert!(plain.contains('3'), "{plain}");
        assert!(matches!(
            load_pill(ListLoadState::Failed, false),
            Some((_, _, true))
        ));
        Ok(())
    }

    #[test]
    fn active_filter_count() {
        let mut filters = ActiveFilters {
            category: SecretCategoryFilter::All,
            audit: AuditFilter::All,
            search: "  ",
            visible: 4,
            loaded: 4,
        };
        assert_eq!(filters.count(), 0);
        filters.category = SecretCategoryFilter::Passphrase;
        filters.audit = AuditFilter::Weak;
        filters.search = "mail";
        assert_eq!(filters.count(), 3);
    }
}

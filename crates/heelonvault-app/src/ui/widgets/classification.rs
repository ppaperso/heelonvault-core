//! Category and tags of the secret editor, entered as badges.
//!
//! One row, two columns: a single category and up to [`MAX_TAGS`] tags. Typing
//! offers the values already used in the loaded vaults; Enter, Tab or a comma
//! turns the text into a badge. The original `gtk4::Entry` fields stay the
//! canonical values (hidden): saving, loading and unsaved-change detection read
//! them unchanged, while the transient inputs are `gtk4::Text`, which the
//! editor's fingerprint ignores. Storage format is unchanged: the category is a
//! string, tags a comma-separated list.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk4::{glib, prelude::*};
use heelonvault_core::i18n::{I18nArg, tr, tr_args};

use crate::ui::search_text::normalize_search_text;

/// Strict limit on tags added through the editor.
pub(crate) const MAX_TAGS: usize = 5;
const MAX_CATEGORY_CHARS: i32 = 40;
const MAX_TAG_CHARS: i32 = 32;
const MAX_SUGGESTIONS: usize = 6;

fn key(value: &str) -> String {
    normalize_search_text(value)
}

/// Existing values offered while typing, most used first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Suggestions {
    pub(crate) categories: Vec<String>,
    pub(crate) tags: Vec<String>,
}

impl Suggestions {
    /// Builds suggestions from `(category, tags)` pairs of loaded secrets.
    /// Values equal after normalization are merged, keeping the first spelling.
    pub(crate) fn collect<'a>(items: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut categories = Ranking::default();
        let mut tags = Ranking::default();
        for (category, raw_tags) in items {
            categories.add(category.trim());
            for tag in parse_tags(raw_tags) {
                tags.add(&tag);
            }
        }
        Self {
            categories: categories.ranked(),
            tags: tags.ranked(),
        }
    }
}

#[derive(Default)]
struct Ranking(HashMap<String, (String, usize, usize)>);

impl Ranking {
    fn add(&mut self, value: &str) {
        let normalized = key(value);
        if normalized.is_empty() {
            return;
        }
        let order = self.0.len();
        let entry = self
            .0
            .entry(normalized)
            .or_insert_with(|| (value.to_string(), 0, order));
        entry.1 += 1;
    }

    fn ranked(self) -> Vec<String> {
        let mut values: Vec<_> = self.0.into_values().collect();
        values.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
        values.into_iter().map(|(value, _, _)| value).collect()
    }
}

/// Splits a stored tag list; blanks and duplicates (after normalization) are dropped.
pub(crate) fn parse_tags(raw: &str) -> Vec<String> {
    let mut seen = Vec::new();
    let mut tags = Vec::new();
    for tag in raw.split(',').map(str::trim).filter(|tag| !tag.is_empty()) {
        let normalized = key(tag);
        if !normalized.is_empty() && !seen.contains(&normalized) {
            seen.push(normalized);
            tags.push(tag.to_string());
        }
    }
    tags
}

pub(crate) fn join_tags(tags: &[String]) -> String {
    tags.join(", ")
}

/// Existing spelling of a typed value, so "travail" reuses "Travail".
fn canonical(candidates: &[String], typed: &str) -> String {
    let typed_key = key(typed);
    candidates
        .iter()
        .find(|candidate| key(candidate) == typed_key)
        .cloned()
        .unwrap_or_else(|| typed.trim().to_string())
}

/// Candidates containing the typed text (prefix matches first), minus `exclude`.
fn matches(candidates: &[String], typed: &str, exclude: &[String]) -> Vec<String> {
    let typed_key = key(typed);
    if typed_key.is_empty() {
        return Vec::new();
    }
    let excluded: Vec<String> = exclude.iter().map(|value| key(value)).collect();
    let mut prefix = Vec::new();
    let mut inner = Vec::new();
    for candidate in candidates {
        let candidate_key = key(candidate);
        if excluded.contains(&candidate_key) {
            continue;
        }
        if candidate_key.starts_with(&typed_key) {
            prefix.push(candidate.clone());
        } else if candidate_key.contains(&typed_key) {
            inner.push(candidate.clone());
        }
    }
    prefix.extend(inner);
    prefix.truncate(MAX_SUGGESTIONS);
    prefix
}

/// Badge with a remove button.
fn chip(value: &str, remove: impl Fn() + 'static) -> gtk4::Box {
    let root = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    root.add_css_class("classification-chip");
    root.set_valign(gtk4::Align::Center);
    let label = gtk4::Label::new(Some(value));
    label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    label.set_max_width_chars(18);
    label.set_tooltip_text(Some(value));
    let button = gtk4::Button::from_icon_name("window-close-symbolic");
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.add_css_class("classification-chip-remove");
    button.set_tooltip_text(Some(&tr_args(
        "add-edit-chip-remove",
        &[("value", I18nArg::Str(value))],
    )));
    button.connect_clicked(move |_| remove());
    root.append(&label);
    root.append(&button);
    root
}

/// Text input with its own suggestion popover (non-modal: typing keeps focus).
struct Completion {
    wrapper: gtk4::Box,
    input: gtk4::Text,
    popover: gtk4::Popover,
    list: gtk4::ListBox,
    selected: Cell<Option<i32>>,
}

impl Completion {
    fn new(max_chars: i32) -> Self {
        let wrapper = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        wrapper.set_hexpand(true);
        wrapper.set_valign(gtk4::Align::Center);
        let input = gtk4::Text::new();
        input.set_max_length(max_chars);
        input.set_hexpand(true);
        input.add_css_class("classification-input");
        wrapper.append(&input);
        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::Single);
        list.set_can_focus(false);
        list.add_css_class("classification-suggestions");
        let popover = gtk4::Popover::new();
        popover.set_autohide(false);
        popover.set_has_arrow(false);
        popover.set_position(gtk4::PositionType::Bottom);
        popover.set_can_focus(false);
        popover.set_child(Some(&list));
        popover.set_parent(&wrapper);
        let owned = popover.clone();
        wrapper.connect_destroy(move |_| owned.unparent());
        Self {
            wrapper,
            input,
            popover,
            list,
            selected: Cell::new(None),
        }
    }

    fn show(&self, values: &[String]) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        self.selected.set(None);
        for value in values {
            let label = gtk4::Label::new(Some(value));
            label.set_xalign(0.0);
            label.set_margin_start(8);
            label.set_margin_end(8);
            label.set_margin_top(4);
            label.set_margin_bottom(4);
            let row = gtk4::ListBoxRow::new();
            row.set_can_focus(false);
            row.set_child(Some(&label));
            self.list.append(&row);
        }
        if values.is_empty() || !self.input.has_focus() || !self.wrapper.is_mapped() {
            self.popover.popdown();
        } else {
            self.popover.popup();
        }
    }

    fn hide(&self) {
        self.selected.set(None);
        self.popover.popdown();
    }

    fn row_text(row: &gtk4::ListBoxRow) -> Option<String> {
        row.child()
            .and_downcast::<gtk4::Label>()
            .map(|label| label.text().to_string())
    }

    fn selected_text(&self) -> Option<String> {
        let index = self.selected.get()?;
        self.list
            .row_at_index(index)
            .and_then(|row| Self::row_text(&row))
    }

    /// Moves the highlighted suggestion; `true` when the key was used.
    fn step(&self, delta: i32) -> bool {
        if !self.popover.is_visible() {
            return false;
        }
        let mut count = 0;
        while self.list.row_at_index(count).is_some() {
            count += 1;
        }
        if count == 0 {
            return false;
        }
        let next = match self.selected.get() {
            None if delta > 0 => 0,
            None => count - 1,
            Some(index) => (index + delta).rem_euclid(count),
        };
        self.selected.set(Some(next));
        self.list.select_row(self.list.row_at_index(next).as_ref());
        true
    }
}

struct State {
    category: gtk4::Entry,
    tags: gtk4::Entry,
    category_label: gtk4::Label,
    tags_label: gtk4::Label,
    counter: gtk4::Label,
    help: gtk4::Button,
    help_text: gtk4::Label,
    category_chip_slot: gtk4::Box,
    category_completion: Completion,
    tags_flow: gtk4::FlowBox,
    tags_completion: Completion,
    suggestions: RefCell<Suggestions>,
}

impl State {
    fn current_tags(&self) -> Vec<String> {
        parse_tags(self.tags.text().as_str())
    }

    fn refresh_i18n(&self) {
        self.category_label
            .set_text(&tr("add-edit-field-category-label"));
        self.tags_label.set_text(&tr("add-edit-field-tags-short"));
        self.help
            .set_tooltip_text(Some(&tr("add-edit-classification-help-tooltip")));
        self.help_text.set_text(&format!(
            "{}\n\n{}\n\n{}",
            tr("add-edit-classification-help-category"),
            tr("add-edit-classification-help-tags"),
            tr("add-edit-classification-help-search"),
        ));
        self.category_completion
            .input
            .set_placeholder_text(Some(&tr("add-edit-category-input-placeholder")));
        self.tags_completion
            .input
            .set_placeholder_text(Some(&tr("add-edit-tags-input-placeholder")));
        self.render_category();
        self.render_tags();
    }

    fn render_category(&self) {
        while let Some(child) = self.category_chip_slot.first_child() {
            self.category_chip_slot.remove(&child);
        }
        let value = self.category.text().trim().to_string();
        let empty = value.is_empty();
        if !empty {
            let entry = self.category.downgrade();
            let input = self.category_completion.input.downgrade();
            let badge = chip(&value, move || {
                if let Some(entry) = entry.upgrade() {
                    entry.set_text("");
                }
                if let Some(input) = input.upgrade() {
                    input.grab_focus();
                }
            });
            badge.add_css_class("classification-chip-category");
            self.category_chip_slot.append(&badge);
        }
        self.category_chip_slot.set_visible(!empty);
        self.category_completion.wrapper.set_visible(empty);
        if !empty {
            self.category_completion.hide();
        }
    }

    fn render_tags(&self) {
        while let Some(child) = self.tags_flow.first_child() {
            self.tags_flow.remove(&child);
        }
        let tags = self.current_tags();
        for (index, tag) in tags.iter().enumerate() {
            let entry = self.tags.downgrade();
            let input = self.tags_completion.input.downgrade();
            let badge = chip(tag, move || {
                if let Some(entry) = entry.upgrade() {
                    let mut tags = parse_tags(entry.text().as_str());
                    if index < tags.len() {
                        tags.remove(index);
                    }
                    entry.set_text(&join_tags(&tags));
                }
                if let Some(input) = input.upgrade() {
                    input.grab_focus();
                }
            });
            self.tags_flow.insert(&badge, -1);
        }
        self.tags_flow.set_visible(!tags.is_empty());
        let full = tags.len() >= MAX_TAGS;
        self.counter.set_text(&tr_args(
            "add-edit-tags-count",
            &[
                ("count", I18nArg::Num(tags.len() as i64)),
                ("max", I18nArg::Num(MAX_TAGS as i64)),
            ],
        ));
        if full {
            self.counter.add_css_class("classification-counter-full");
            self.counter
                .set_tooltip_text(Some(&tr("add-edit-tags-limit")));
            self.tags_completion.input.set_text("");
            self.tags_completion.hide();
        } else {
            self.counter.remove_css_class("classification-counter-full");
            self.counter.set_tooltip_text(None);
        }
        self.tags_completion.wrapper.set_visible(!full);
    }

    fn commit_category(&self, typed: &str) {
        let typed = typed.trim().trim_matches(',').trim();
        self.category_completion.input.set_text("");
        if typed.is_empty() {
            return;
        }
        let value = canonical(&self.suggestions.borrow().categories, typed);
        self.category.set_text(&value);
    }

    /// Adds every comma-separated part, up to the strict limit; returns how many were added.
    fn commit_tags(&self, typed: &str) -> usize {
        self.tags_completion.input.set_text("");
        let mut tags = self.current_tags();
        let mut added = 0;
        for part in typed
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if tags.len() >= MAX_TAGS {
                break;
            }
            let value = canonical(&self.suggestions.borrow().tags, part);
            if !tags.iter().any(|tag| key(tag) == key(&value)) {
                tags.push(value);
                added += 1;
            }
        }
        if added > 0 {
            self.tags.set_text(&join_tags(&tags));
        }
        added
    }

    fn suggest_category(&self) {
        let typed = self.category_completion.input.text();
        let values = matches(&self.suggestions.borrow().categories, &typed, &[]);
        self.category_completion.show(&values);
    }

    fn suggest_tags(&self) {
        let typed = self.tags_completion.input.text();
        let values = matches(
            &self.suggestions.borrow().tags,
            &typed,
            &self.current_tags(),
        );
        self.tags_completion.show(&values);
    }
}

/// Handle kept by the editor: suggestions and live retranslation.
#[derive(Clone)]
pub(crate) struct ClassificationField {
    pub(crate) root: gtk4::Box,
    state: Weak<State>,
}

impl ClassificationField {
    pub(crate) fn set_suggestions(&self, suggestions: Suggestions) {
        if let Some(state) = self.state.upgrade() {
            *state.suggestions.borrow_mut() = suggestions;
        }
    }

    pub(crate) fn refresh_i18n(&self) {
        if let Some(state) = self.state.upgrade() {
            state.refresh_i18n();
        }
    }

    #[cfg(test)]
    fn state(&self) -> Option<Rc<State>> {
        self.state.upgrade()
    }
}

fn field_frame() -> gtk4::Box {
    let frame = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    frame.add_css_class("classification-field");
    frame.set_valign(gtk4::Align::Start);
    frame
}

fn column_label() -> gtk4::Label {
    let label = gtk4::Label::new(None);
    label.add_css_class("field-label");
    label.set_halign(gtk4::Align::Start);
    label
}

/// Keyboard: Enter/Tab/comma commit, Up/Down pick a suggestion, Escape closes
/// the list, Backspace on an empty tag input removes the last tag.
fn wire_keys(state: &Rc<State>, tags: bool) {
    let completion = if tags {
        &state.tags_completion
    } else {
        &state.category_completion
    };
    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let weak = Rc::downgrade(state);
    keys.connect_key_pressed(move |_, key, _, _| {
        use gtk4::gdk::Key;
        let Some(state) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let completion = if tags {
            &state.tags_completion
        } else {
            &state.category_completion
        };
        let text = completion.input.text().to_string();
        let commit = |value: &str| {
            if tags {
                state.commit_tags(value);
            } else {
                state.commit_category(value);
            }
            completion.hide();
        };
        match key {
            Key::Down => glib::Propagation::from(completion.step(1)),
            Key::Up => glib::Propagation::from(completion.step(-1)),
            Key::Escape if completion.popover.is_visible() => {
                completion.hide();
                glib::Propagation::Stop
            }
            Key::Return | Key::KP_Enter | Key::ISO_Enter => {
                let value = completion.selected_text().unwrap_or(text);
                commit(&value);
                glib::Propagation::Stop
            }
            Key::Tab | Key::comma if !text.trim().is_empty() => {
                let value = completion.selected_text().unwrap_or(text);
                commit(&value);
                glib::Propagation::Stop
            }
            Key::comma => glib::Propagation::Stop,
            Key::BackSpace if tags && text.is_empty() => {
                let mut current = state.current_tags();
                if current.pop().is_some() {
                    state.tags.set_text(&join_tags(&current));
                }
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    completion.input.add_controller(keys);

    let weak = Rc::downgrade(state);
    completion.input.connect_changed(move |_| {
        if let Some(state) = weak.upgrade() {
            if tags {
                state.suggest_tags();
            } else {
                state.suggest_category();
            }
        }
    });
    let weak = Rc::downgrade(state);
    completion.list.connect_row_activated(move |_, row| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        if let Some(value) = Completion::row_text(row) {
            if tags {
                state.commit_tags(&value);
                state.tags_completion.hide();
                state.tags_completion.input.grab_focus();
            } else {
                state.commit_category(&value);
                state.category_completion.hide();
            }
        }
    });
    let focus = gtk4::EventControllerFocus::new();
    let weak = Rc::downgrade(state);
    focus.connect_leave(move |_| {
        if let Some(state) = weak.upgrade() {
            if tags {
                state.tags_completion.hide();
            } else {
                state.category_completion.hide();
            }
        }
    });
    completion.input.add_controller(focus);
}

fn build_help() -> (gtk4::Button, gtk4::Label) {
    let text = gtk4::Label::new(None);
    text.set_wrap(true);
    text.set_max_width_chars(44);
    text.set_xalign(0.0);
    text.add_css_class("classification-help-text");
    let popover = gtk4::Popover::new();
    popover.set_child(Some(&text));
    // A plain "?" button: a MenuButton label would add a dropdown arrow.
    let button = gtk4::Button::with_label("?");
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.add_css_class("classification-help");
    button.set_valign(gtk4::Align::Center);
    popover.set_parent(&button);
    let owned = popover.clone();
    button.connect_clicked(move |_| owned.popup());
    let owned = popover;
    button.connect_destroy(move |_| owned.unparent());
    (button, text)
}

/// Replaces the two full-width category/tags rows with one compact row.
/// `category` and `tags` are the editor's existing entries, kept as canonical values.
pub(crate) fn build(category: &gtk4::Entry, tags: &gtk4::Entry) -> ClassificationField {
    let root = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    root.add_css_class("classification-row");

    // Category column: label + help, then one badge or the input.
    let category_column = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    category_column.set_hexpand(false);
    category_column.set_size_request(200, -1);
    let category_header = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    category_header.add_css_class("classification-header");
    let category_label = column_label();
    let (help, help_text) = build_help();
    category_header.append(&category_label);
    category_header.append(&help);
    let category_frame = field_frame();
    let category_chip_slot = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    let category_completion = Completion::new(MAX_CATEGORY_CHARS);
    category_frame.append(&category_chip_slot);
    category_frame.append(&category_completion.wrapper);
    category_column.append(&category_header);
    category_column.append(&category_frame);

    // Tags column: label + counter, then badges and the input.
    let tags_column = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    tags_column.set_hexpand(true);
    let tags_header = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    tags_header.add_css_class("classification-header");
    let tags_label = column_label();
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let counter = gtk4::Label::new(None);
    counter.add_css_class("classification-counter");
    tags_header.append(&tags_label);
    tags_header.append(&spacer);
    tags_header.append(&counter);
    let tags_frame = field_frame();
    let tags_flow = gtk4::FlowBox::new();
    tags_flow.set_selection_mode(gtk4::SelectionMode::None);
    tags_flow.set_homogeneous(false);
    tags_flow.set_column_spacing(4);
    tags_flow.set_row_spacing(4);
    tags_flow.set_max_children_per_line(MAX_TAGS as u32);
    tags_flow.set_valign(gtk4::Align::Center);
    let tags_completion = Completion::new(MAX_TAG_CHARS);
    tags_frame.append(&tags_flow);
    tags_frame.append(&tags_completion.wrapper);
    tags_column.append(&tags_header);
    tags_column.append(&tags_frame);

    root.append(&category_column);
    root.append(&tags_column);
    // Canonical values stay in the tree so the editor's fingerprint sees them.
    for entry in [category, tags] {
        entry.set_visible(false);
        root.append(entry);
    }

    let state = Rc::new(State {
        category: category.clone(),
        tags: tags.clone(),
        category_label,
        tags_label,
        counter,
        help,
        help_text,
        category_chip_slot,
        category_completion,
        tags_flow,
        tags_completion,
        suggestions: RefCell::new(Suggestions::default()),
    });
    wire_keys(&state, false);
    wire_keys(&state, true);
    let weak = Rc::downgrade(&state);
    category.connect_changed(move |_| {
        if let Some(state) = weak.upgrade() {
            state.render_category();
        }
    });
    let weak = Rc::downgrade(&state);
    tags.connect_changed(move |_| {
        if let Some(state) = weak.upgrade() {
            state.render_tags();
        }
    });
    state.refresh_i18n();
    // Signal handlers hold weak references; the row keeps the state alive.
    let keep = Rc::clone(&state);
    root.connect_destroy(move |_| {
        keep.category_completion.hide();
        keep.tags_completion.hide();
    });
    ClassificationField {
        root,
        state: Rc::downgrade(&state),
    }
}

/// Native GTK check of the classification row (run from `native_gtk_smoke`).
#[cfg(test)]
pub(crate) fn native_classification_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let category = gtk4::Entry::new();
    let tags = gtk4::Entry::new();
    let field = build(&category, &tags);
    let state = field.state().ok_or("classification state dropped")?;
    field.set_suggestions(Suggestions::collect([
        ("Travail", "prod, client-a"),
        ("travail", "Prod, finance"),
        ("Personnel", ""),
    ]));
    assert!(!WidgetExt::is_visible(&category) && !WidgetExt::is_visible(&tags));
    assert!(state.category_completion.wrapper.is_visible());

    // Category: typed text reuses the existing spelling and becomes one badge.
    state.commit_category("TRAVAIL");
    assert_eq!(category.text(), "Travail");
    assert!(state.category_chip_slot.is_visible());
    assert!(!state.category_completion.wrapper.is_visible());
    category.set_text("");
    assert!(state.category_completion.wrapper.is_visible());

    // Tags: commas split, duplicates are merged, the limit is strict.
    assert_eq!(state.commit_tags("prod, nouveau,PROD"), 2);
    assert_eq!(tags.text(), "prod, nouveau");
    assert_eq!(state.commit_tags("a, b, c, d"), 3);
    assert_eq!(state.current_tags().len(), MAX_TAGS);
    assert!(
        !state.tags_completion.wrapper.is_visible(),
        "input hidden at the limit"
    );
    assert!(state.counter.has_css_class("classification-counter-full"));
    assert_eq!(state.commit_tags("extra"), 0);
    assert_eq!(state.current_tags().len(), MAX_TAGS);

    // A legacy secret above the limit keeps every tag (nothing dropped silently).
    tags.set_text("a, b, c, d, e, f, g");
    assert_eq!(state.current_tags().len(), 7);
    assert!(!state.tags_completion.wrapper.is_visible());
    tags.set_text("a");
    assert!(state.tags_completion.wrapper.is_visible());

    let language = heelonvault_core::i18n::current_language();
    for lang in ["fr", "en"] {
        assert!(heelonvault_core::i18n::set_language(lang));
        field.refresh_i18n();
        assert_eq!(state.tags_label.text(), tr("add-edit-field-tags-short"));
        assert!(
            state
                .help_text
                .text()
                .contains(&tr("add-edit-classification-help-tags"))
        );
    }
    assert!(heelonvault_core::i18n::set_language(&language));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_parsed_trimmed_and_deduplicated() {
        assert_eq!(
            parse_tags(" prod ,client-a,, Prod , Équipe"),
            vec!["prod", "client-a", "Équipe"]
        );
        assert_eq!(parse_tags(""), Vec::<String>::new());
        assert_eq!(join_tags(&parse_tags("a,b")), "a, b");
    }

    #[test]
    fn suggestions_merge_spellings_and_rank_by_use() {
        let suggestions = Suggestions::collect([
            ("Travail", "prod, client-a"),
            ("travail", "Prod"),
            ("Personnel", "perso"),
            ("", "prod"),
        ]);
        assert_eq!(suggestions.categories, vec!["Travail", "Personnel"]);
        assert_eq!(suggestions.tags[0], "prod");
        assert_eq!(suggestions.tags.len(), 3);
    }

    #[test]
    fn typing_matches_prefix_first_and_skips_chosen_tags() {
        let candidates = vec![
            "finance".to_string(),
            "prod".to_string(),
            "preprod".to_string(),
        ];
        assert_eq!(matches(&candidates, "pro", &[]), vec!["prod", "preprod"]);
        assert_eq!(
            matches(&candidates, "PRO", &["prod".into()]),
            vec!["preprod"]
        );
        assert!(matches(&candidates, "", &[]).is_empty());
        assert_eq!(canonical(&candidates, " FINANCE "), "finance");
        assert_eq!(canonical(&candidates, "nouveau"), "nouveau");
    }

    #[test]
    fn every_classification_key_exists_in_both_catalogs() {
        let keys = [
            "add-edit-field-category-label",
            "add-edit-field-tags-short",
            "add-edit-classification-help-tooltip",
            "add-edit-classification-help-category",
            "add-edit-classification-help-tags",
            "add-edit-classification-help-search",
            "add-edit-category-input-placeholder",
            "add-edit-tags-input-placeholder",
            "add-edit-tags-count",
            "add-edit-tags-limit",
            "add-edit-chip-remove",
        ];
        for lang in ["fr", "en"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../heelonvault-core/locales")
                .join(lang)
                .join("main.ftl");
            let catalog = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(!catalog.is_empty(), "cannot read {}", path.display());
            for key in keys {
                let defined = catalog
                    .lines()
                    .any(|line| line.split_once(" =").is_some_and(|(id, _)| id == key));
                assert!(defined, "{key} missing in {lang}");
            }
        }
    }
}

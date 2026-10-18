use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use heelonvault_core::i18n::{I18nArg, tr, tr_args};
use heelonvault_core::services::security_center::assess_password;
use libadwaita as adw;
use libadwaita::prelude::*;

// ─── ANSSI complexity criteria ──────────────────────────────────────────────

struct Criteria {
    has_upper: bool,
    has_lower: bool,
    has_digit: bool,
    has_special: bool,
}

impl Criteria {
    fn check(password: &str) -> Self {
        Self {
            has_upper: password.chars().any(|c| c.is_uppercase()),
            has_lower: password.chars().any(|c| c.is_lowercase()),
            has_digit: password.chars().any(|c| c.is_ascii_digit()),
            has_special: password.chars().any(|c| !c.is_alphanumeric()),
        }
    }

    fn category_count(&self) -> usize {
        [
            self.has_upper,
            self.has_lower,
            self.has_digit,
            self.has_special,
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }

    #[allow(dead_code)]
    fn all_met(&self) -> bool {
        self.category_count() == 4
    }
}

// The ANSSI-aligned policy cap (length × character classes) lives in
// `heelonvault_core::services::security_center`, shared with the security center and the list.

/// Human-readable label for the ANSSI policy score.
fn anssi_hint(len: usize, categories: usize, zxcvbn_score: i32) -> String {
    if len < 12 {
        return tr_args(
            "password-hint-too-short",
            &[(
                "count",
                I18nArg::Num(i64::try_from(len).unwrap_or(i64::MAX)),
            )],
        );
    }
    if categories < 3 {
        let missing = 3usize.saturating_sub(categories);
        return tr_args(
            "password-hint-missing-categories",
            &[(
                "count",
                I18nArg::Num(i64::try_from(missing).unwrap_or(i64::MAX)),
            )],
        );
    }
    if len < 15 && categories < 4 {
        if len < 14 {
            return tr("password-hint-for-solid");
        }
        return tr("password-hint-for-robust");
    }
    // All ANSSI requirements met — relay zxcvbn signal
    tr(match zxcvbn_score {
        0 | 1 => "password-hint-score-good",
        2 => "password-hint-score-medium",
        3 => "password-hint-score-solid",
        _ => "password-hint-score-robust",
    })
}

// ─── Shared widget state ────────────────────────────────────────────────────

struct StrengthState {
    root: gtk4::Box,
    level: gtk4::LevelBar,
    hint: gtk4::Label,
    badge_upper: gtk4::Label,
    badge_lower: gtk4::Label,
    badge_digit: gtk4::Label,
    badge_spec: gtk4::Label,
    username: RefCell<String>,
    trim_input: Cell<bool>,
    /// Score observed at the last update — written by update_view, read by gate_button.
    last_score: RefCell<i32>,
}

// ─── Public widget ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PasswordStrengthBar {
    state: Rc<StrengthState>,
}

impl Default for PasswordStrengthBar {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordStrengthBar {
    pub fn new() -> Self {
        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(6)
            .margin_top(4)
            .margin_bottom(4)
            .build();
        root.add_css_class("password-strength-root");
        root.set_visible(false);

        let level = gtk4::LevelBar::for_interval(0.0, 4.0);
        level.set_value(0.0);
        level.set_hexpand(true);
        level.add_css_class("password-strength-bar");
        // Override default offsets so that 1=low, 2–3=medium, 4=high
        level.add_offset_value("low", 1.0);
        level.add_offset_value("medium", 3.0);
        level.add_offset_value("high", 4.0);

        // — Badge row (abc / ABC / 123 / #?!) ──────────────────────────
        let badge_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .margin_top(2)
            .build();

        let make_badge = |text: &str| -> gtk4::Label {
            let label = gtk4::Label::new(Some(text));
            label.set_halign(gtk4::Align::Center);
            label.set_hexpand(true);
            label.add_css_class("psb-badge");
            label.add_css_class("psb-badge-off");
            label
        };

        let badge_lower = make_badge("abc");
        let badge_upper = make_badge("ABC");
        let badge_digit = make_badge("123");
        let badge_spec = make_badge("#?!");

        badge_row.append(&badge_lower);
        badge_row.append(&badge_upper);
        badge_row.append(&badge_digit);
        badge_row.append(&badge_spec);

        // — Dynamic hint ───────────────────────────────────────────────
        let hint = gtk4::Label::new(None);
        hint.set_halign(gtk4::Align::Start);
        hint.set_wrap(true);
        hint.add_css_class("caption");
        hint.add_css_class("password-strength-hint");

        root.append(&level);
        root.append(&badge_row);
        root.append(&hint);

        Self {
            state: Rc::new(StrengthState {
                root,
                level,
                hint,
                badge_upper,
                badge_lower,
                badge_digit,
                badge_spec,
                username: RefCell::new(String::new()),
                trim_input: Cell::new(true),
                last_score: RefCell::new(0),
            }),
        }
    }

    /// Current policy score (0–4). Useful to gate a button externally.
    pub fn last_score(&self) -> i32 {
        *self.state.last_score.borrow()
    }

    pub fn for_stored_passwords() -> Self {
        let bar = Self::new();
        bar.state.trim_input.set(false);
        bar
    }

    /// Feed the current username so zxcvbn penalises it in the password.
    // Phase 5a: not yet wired to login/edit dialogs. Owner: ppaadmin | Due: Phase 5b
    #[allow(dead_code)]
    pub fn set_username(&self, username: &str) {
        *self.state.username.borrow_mut() = username.to_ascii_lowercase();
    }

    pub fn root(&self) -> &gtk4::Box {
        &self.state.root
    }

    // Phase 5a: not yet wired. Owner: ppaadmin | Due: Phase 5b
    #[allow(dead_code)]
    pub fn as_action_row(&self) -> adw::ActionRow {
        let row = adw::ActionRow::new();
        row.set_activatable(false);
        row.add_css_class("password-strength-row");
        row.add_suffix(self.root());
        row
    }

    /// Bind to an entry **and** gate `button`: the button stays insensitive
    /// until the policy score is ≥ `min_score` (3 = Solide).
    // Phase 5a: not yet wired. Owner: ppaadmin | Due: Phase 5b
    #[allow(dead_code)]
    pub fn connect_and_gate_button<E>(&self, entry: &E, button: &gtk4::Button, min_score: i32)
    where
        E: IsA<gtk4::Editable> + Clone + 'static,
    {
        self.bind_editable(entry);
        // Connect button sensitivity to score changes.
        let state = Rc::clone(&self.state);
        let btn_ref = button.clone();
        entry.connect_text_notify(move |_| {
            let ok = *state.last_score.borrow() >= min_score;
            btn_ref.set_sensitive(ok);
        });
        // Apply immediately.
        button.set_sensitive(*self.state.last_score.borrow() >= min_score);
    }

    // Phase 5a: not yet wired. Owner: ppaadmin | Due: Phase 5b
    #[allow(dead_code)]
    pub fn connect_to_entry(&self, entry: &adw::EntryRow) {
        self.bind_editable(entry);
    }

    // Phase 5a: not yet wired. Owner: ppaadmin | Due: Phase 5b
    #[allow(dead_code)]
    pub fn connect_to_password_entry_row(&self, entry: &adw::PasswordEntryRow) {
        self.bind_editable(entry);
    }

    pub fn connect_to_password_entry(&self, entry: &gtk4::PasswordEntry) {
        self.bind_editable(entry);
    }

    fn bind_editable<E: IsA<gtk4::Editable> + Clone + 'static>(&self, entry: &E) {
        let state = Rc::clone(&self.state);
        entry.connect_text_notify(move |editable| {
            let password = editable.text().to_string();
            let username = state.username.borrow().clone();
            Self::update_view(&state, &password, &username);
        });
        let username = self.state.username.borrow().clone();
        Self::update_view(&self.state, entry.text().as_str(), &username);
    }

    // ── Badge helpers ─────────────────────────────────────────────────────

    fn set_badge(label: &gtk4::Label, active: bool) {
        if active {
            label.remove_css_class("psb-badge-off");
            label.add_css_class("psb-badge-on");
        } else {
            label.remove_css_class("psb-badge-on");
            label.add_css_class("psb-badge-off");
        }
    }

    // ── Core logic ────────────────────────────────────────────────────────

    fn update_view(state: &StrengthState, password: &str, username: &str) {
        let trimmed = if state.trim_input.get() {
            password.trim()
        } else {
            password
        };
        if trimmed.is_empty() {
            state.root.set_visible(false);
            state.level.set_value(0.0);
            state.hint.set_label("");
            *state.last_score.borrow_mut() = 0;
            return;
        }

        state.root.set_visible(true);
        let len = trimmed.chars().count();

        // 1 — Criteria / badges
        let crit = Criteria::check(trimmed);
        Self::set_badge(&state.badge_lower, crit.has_lower);
        Self::set_badge(&state.badge_upper, crit.has_upper);
        Self::set_badge(&state.badge_digit, crit.has_digit);
        Self::set_badge(&state.badge_spec, crit.has_special);
        let cats = crit.category_count();

        // 2 — zxcvbn with full blocklist
        let assessment = assess_password(trimmed, username);
        let zxcvbn_score = i32::from(assessment.estimator_score);

        // 3 — Final score: min(ANSSI cap, zxcvbn)
        let cap = i32::from(assessment.policy_cap);
        let score = i32::from(assessment.score);
        state.level.set_value(f64::from(score));
        *state.last_score.borrow_mut() = score;

        // 4 — Hint
        let hint = anssi_hint(len, cats, zxcvbn_score);
        // Append a zxcvbn warning on top of the ANSSI hint when criteria are met
        let hint = if cap >= 4 {
            if let Some(extra) = &assessment.feedback {
                format!("{hint} — {}", Self::localize(extra))
            } else {
                hint
            }
        } else {
            hint
        };
        state.hint.set_label(&hint);
    }

    /// Translate a zxcvbn feedback message (English) into the active language.
    fn localize(message: &str) -> String {
        let n = message.to_ascii_lowercase();
        let key = if n.contains("too short") {
            "password-feedback-too-short"
        } else if n.contains("similar") {
            "password-feedback-similar"
        } else if n.contains("repeat") {
            "password-feedback-repeats"
        } else if n.contains("sequence") {
            "password-feedback-sequence"
        } else if n.contains("common") {
            "password-feedback-common"
        } else if n.contains("word") {
            "password-feedback-add-symbols"
        } else {
            "password-feedback-generic"
        };
        tr(key)
    }
}

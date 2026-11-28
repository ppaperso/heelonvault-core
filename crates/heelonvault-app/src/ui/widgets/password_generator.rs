//! Password / passphrase generator panel of the secret editor.
//!
//! The panel shows a candidate (monospace preview, digits and symbols colored), its entropy
//! and ANSSI level, and the options of the selected kind. "Use" copies the candidate into the
//! target entry, after an inline confirmation when it would replace a different value.
//! Recipes come from `heelonvault_core::services::password_generator`; the candidate is wiped
//! when the panel closes, after use, and on security clearing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use heelonvault_core::i18n::{I18nArg, tr_args};
use heelonvault_core::models::PasswordKind;
use heelonvault_core::services::password_generator::{
    EntropyLevel, GeneratorRecipe, MAX_PASSPHRASE_EXTRAS, MAX_PASSPHRASE_WORDS,
    MAX_PASSWORD_RECIPE_LENGTH, MIN_PASSPHRASE_WORDS, PassphraseCapitalization, PassphraseRecipe,
    PassphraseSeparator, PasswordRecipe, WordListLanguage,
};
use secrecy::{ExposeSecret, SecretBox};
use zeroize::Zeroizing;

use crate::ui::view_preferences;

/// The editor only saves a new password whose policy score is 4, which the length/class cap
/// allows from 15 characters on: shorter generated passwords could not be saved.
const STORED_PASSWORD_MIN_LENGTH: u16 = 15;

/// CSS class excluding a subtree from the editor's unsaved-change fingerprint: generator
/// settings are not part of the secret.
pub const FINGERPRINT_SKIP_CLASS: &str = "editor-fingerprint-skip";

const LANGUAGES: [WordListLanguage; 2] = [WordListLanguage::English, WordListLanguage::French];
const SEPARATORS: [PassphraseSeparator; 5] = [
    PassphraseSeparator::Hyphen,
    PassphraseSeparator::Dot,
    PassphraseSeparator::Underscore,
    PassphraseSeparator::Space,
    PassphraseSeparator::RandomDigitOrSymbol,
];
const CAPITALIZATIONS: [PassphraseCapitalization; 3] = [
    PassphraseCapitalization::None,
    PassphraseCapitalization::FirstLetter,
    PassphraseCapitalization::Random,
];

struct Inner {
    revealer: gtk4::Revealer,
    target: glib::WeakRef<gtk4::PasswordEntry>,
    kind: Cell<PasswordKind>,
    preview: gtk4::Label,
    entropy: gtk4::Label,
    regenerate: gtk4::Button,
    use_button: gtk4::Button,
    options: gtk4::Stack,
    // Password options.
    length_title: gtk4::Label,
    length: gtk4::Adjustment,
    length_hint: gtk4::Label,
    lowercase: gtk4::CheckButton,
    uppercase: gtk4::CheckButton,
    digits: gtk4::CheckButton,
    symbols: gtk4::CheckButton,
    exclude_ambiguous: gtk4::CheckButton,
    // Passphrase options.
    words_title: gtk4::Label,
    words: gtk4::Adjustment,
    words_hint: gtk4::Label,
    language_title: gtk4::Label,
    language: gtk4::DropDown,
    separator_title: gtk4::Label,
    separator: gtk4::DropDown,
    capitalization_title: gtk4::Label,
    capitalization: gtk4::DropDown,
    extra_digits_title: gtk4::Label,
    extra_digits: gtk4::SpinButton,
    extra_symbols_title: gtk4::Label,
    extra_symbols: gtk4::SpinButton,
    // Inline replacement confirmation.
    confirm: gtk4::Revealer,
    confirm_label: gtk4::Label,
    confirm_yes: gtk4::Button,
    confirm_no: gtk4::Button,
    candidate: RefCell<Option<SecretBox<Vec<u8>>>>,
    /// Recipe of the value last copied into the entry, until the entry is edited by hand.
    applied: RefCell<Option<GeneratorRecipe>>,
    applying: Cell<bool>,
    /// Set while widgets are updated programmatically, to avoid cascaded regenerations.
    syncing: Cell<bool>,
}

#[derive(Clone)]
pub struct PasswordGenerator {
    inner: Rc<Inner>,
}

fn title_label() -> gtk4::Label {
    let label = gtk4::Label::new(None);
    label.set_halign(gtk4::Align::Start);
    label.add_css_class("field-label");
    label
}

fn hint_label() -> gtk4::Label {
    let label = gtk4::Label::new(None);
    label.set_halign(gtk4::Align::Start);
    label.set_wrap(true);
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    label
}

fn index_of<T: PartialEq>(values: &[T], value: &T) -> u32 {
    values
        .iter()
        .position(|candidate| candidate == value)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

fn value_at<T: Copy + Default>(values: &[T], index: u32) -> T {
    usize::try_from(index)
        .ok()
        .and_then(|index| values.get(index).copied())
        .unwrap_or_default()
}

fn set_strings(dropdown: &gtk4::DropDown, strings: &[String]) {
    let selected = dropdown.selected();
    let refs: Vec<&str> = strings.iter().map(String::as_str).collect();
    dropdown.set_model(Some(&gtk4::StringList::new(&refs)));
    dropdown.set_selected(selected);
}

/// Pango markup of a generated value: digits and symbols colored for readability.
fn preview_markup(value: &[u8]) -> Zeroizing<String> {
    // Worst case per byte: span opening (27) + escape (6) + span closing (7); sized up front so
    // the buffer never reallocates (a reallocation would leave an unzeroized copy).
    let mut markup = Zeroizing::new(String::with_capacity(value.len() * 40));
    // Characters are pushed one by one into the zeroized buffer (no temporary strings).
    let push_escaped = |markup: &mut String, byte: u8| match byte {
        b'&' => markup.push_str("&amp;"),
        b'<' => markup.push_str("&lt;"),
        b'>' => markup.push_str("&gt;"),
        b'\'' => markup.push_str("&apos;"),
        b'"' => markup.push_str("&quot;"),
        b' ' => markup.push('\u{2423}'),
        other => markup.push(char::from(other)),
    };
    for byte in value {
        let color = if byte.is_ascii_digit() {
            Some("#3584e4")
        } else if !byte.is_ascii_alphanumeric() {
            Some("#e66100")
        } else {
            None
        };
        if let Some(color) = color {
            markup.push_str("<span foreground=\"");
            markup.push_str(color);
            markup.push_str("\">");
            push_escaped(&mut markup, *byte);
            markup.push_str("</span>");
        } else {
            push_escaped(&mut markup, *byte);
        }
    }
    markup
}

impl PasswordGenerator {
    pub fn new(target: &gtk4::PasswordEntry) -> Self {
        // Tests never read nor write the user's real preference file.
        let (saved_password, saved_passphrase) = if cfg!(test) {
            (None, None)
        } else {
            view_preferences::load_generator_recipes()
        };
        let password = saved_password
            .filter(|recipe| recipe.validate().is_ok())
            .unwrap_or_default();
        let passphrase = saved_passphrase
            .filter(|recipe| recipe.validate().is_ok())
            .unwrap_or_else(|| {
                let mut recipe = PassphraseRecipe::default();
                if heelonvault_core::i18n::current_language() == "fr" {
                    recipe.language = WordListLanguage::French;
                }
                recipe
            });

        let panel = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .margin_top(6)
            .build();
        panel.add_css_class("password-generator-panel");
        panel.add_css_class(FINGERPRINT_SKIP_CLASS);

        // Preview + regenerate.
        let preview_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let preview = gtk4::Label::new(None);
        preview.set_hexpand(true);
        preview.set_halign(gtk4::Align::Fill);
        preview.set_xalign(0.0);
        preview.set_wrap(true);
        // Break between passphrase words first, inside long passwords only when needed.
        preview.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        preview.set_width_chars(24);
        preview.set_max_width_chars(64);
        preview.add_css_class("monospace");
        preview.add_css_class("password-generator-preview");
        let regenerate = gtk4::Button::new();
        regenerate.add_css_class("secondary-pill");
        regenerate.set_valign(gtk4::Align::Center);
        preview_row.append(&preview);
        preview_row.append(&regenerate);

        let entropy = gtk4::Label::new(None);
        entropy.set_halign(gtk4::Align::Start);
        entropy.add_css_class("caption-heading");
        entropy.add_css_class("password-generator-entropy");

        // Password options.
        let password_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        let length_title = title_label();
        let length = gtk4::Adjustment::new(
            f64::from(password.length),
            f64::from(STORED_PASSWORD_MIN_LENGTH),
            f64::from(MAX_PASSWORD_RECIPE_LENGTH),
            1.0,
            8.0,
            0.0,
        );
        let length_row = Self::slider_row(&length);
        let length_hint = hint_label();
        let lowercase = gtk4::CheckButton::new();
        lowercase.set_active(password.lowercase);
        let uppercase = gtk4::CheckButton::new();
        uppercase.set_active(password.uppercase);
        let digits = gtk4::CheckButton::new();
        digits.set_active(password.digits);
        let symbols = gtk4::CheckButton::new();
        symbols.set_active(password.symbols);
        let classes = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .max_children_per_line(2)
            .min_children_per_line(1)
            .homogeneous(true)
            .build();
        for check in [&lowercase, &uppercase, &digits, &symbols] {
            classes.insert(check, -1);
        }
        let exclude_ambiguous = gtk4::CheckButton::new();
        exclude_ambiguous.set_active(password.exclude_ambiguous);
        password_box.append(&length_title);
        password_box.append(&length_row);
        password_box.append(&length_hint);
        password_box.append(&classes);
        password_box.append(&exclude_ambiguous);

        // Passphrase options.
        let passphrase_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        let words_title = title_label();
        let words = gtk4::Adjustment::new(
            f64::from(passphrase.words),
            f64::from(MIN_PASSPHRASE_WORDS),
            f64::from(MAX_PASSPHRASE_WORDS),
            1.0,
            2.0,
            0.0,
        );
        let words_row = Self::slider_row(&words);
        let words_hint = hint_label();
        let grid = gtk4::Grid::builder()
            .row_spacing(6)
            .column_spacing(12)
            .build();
        let language_title = title_label();
        let language = gtk4::DropDown::from_strings(&["", ""]);
        language.set_selected(index_of(&LANGUAGES, &passphrase.language));
        let separator_title = title_label();
        let separator = gtk4::DropDown::from_strings(&["", "", "", "", ""]);
        separator.set_selected(index_of(&SEPARATORS, &passphrase.separator));
        let capitalization_title = title_label();
        let capitalization = gtk4::DropDown::from_strings(&["", "", ""]);
        capitalization.set_selected(index_of(&CAPITALIZATIONS, &passphrase.capitalization));
        let extras = f64::from(MAX_PASSPHRASE_EXTRAS);
        let extra_digits_title = title_label();
        let extra_digits = gtk4::SpinButton::with_range(0.0, extras, 1.0);
        extra_digits.set_value(f64::from(passphrase.digits));
        let extra_symbols_title = title_label();
        let extra_symbols = gtk4::SpinButton::with_range(0.0, extras, 1.0);
        extra_symbols.set_value(f64::from(passphrase.symbols));
        let rows: [(&gtk4::Label, &gtk4::Widget); 5] = [
            (&language_title, language.upcast_ref()),
            (&separator_title, separator.upcast_ref()),
            (&capitalization_title, capitalization.upcast_ref()),
            (&extra_digits_title, extra_digits.upcast_ref()),
            (&extra_symbols_title, extra_symbols.upcast_ref()),
        ];
        for (row, (title, widget)) in (0_i32..).zip(rows) {
            title.set_hexpand(true);
            grid.attach(title, 0, row, 1, 1);
            widget.set_halign(gtk4::Align::End);
            grid.attach(widget, 1, row, 1, 1);
        }
        passphrase_box.append(&words_title);
        passphrase_box.append(&words_row);
        passphrase_box.append(&words_hint);
        passphrase_box.append(&grid);

        let options = gtk4::Stack::new();
        options.set_hhomogeneous(true);
        options.set_vhomogeneous(false);
        options.add_named(&password_box, Some("password"));
        options.add_named(&passphrase_box, Some("passphrase"));

        // Inline confirmation + use.
        let confirm_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        confirm_box.add_css_class("password-generator-confirm");
        let confirm_label = gtk4::Label::new(None);
        confirm_label.set_wrap(true);
        confirm_label.set_hexpand(true);
        confirm_label.set_xalign(0.0);
        let confirm_no = gtk4::Button::new();
        confirm_no.add_css_class("flat");
        let confirm_yes = gtk4::Button::new();
        confirm_yes.add_css_class("destructive-action");
        confirm_box.append(&confirm_label);
        confirm_box.append(&confirm_no);
        confirm_box.append(&confirm_yes);
        let confirm = gtk4::Revealer::new();
        confirm.set_child(Some(&confirm_box));

        let use_button = gtk4::Button::new();
        use_button.add_css_class("suggested-action");
        use_button.set_halign(gtk4::Align::End);

        panel.append(&preview_row);
        panel.append(&entropy);
        panel.append(&options);
        panel.append(&confirm);
        panel.append(&use_button);

        let revealer = gtk4::Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::SlideDown)
            .reveal_child(false)
            .build();
        revealer.add_css_class(FINGERPRINT_SKIP_CLASS);
        revealer.set_child(Some(&panel));

        let generator = Self {
            inner: Rc::new(Inner {
                revealer,
                target: target.downgrade(),
                kind: Cell::new(PasswordKind::Password),
                preview,
                entropy,
                regenerate,
                use_button,
                options,
                length_title,
                length,
                length_hint,
                lowercase,
                uppercase,
                digits,
                symbols,
                exclude_ambiguous,
                words_title,
                words,
                words_hint,
                language_title,
                language,
                separator_title,
                separator,
                capitalization_title,
                capitalization,
                extra_digits_title,
                extra_digits,
                extra_symbols_title,
                extra_symbols,
                confirm,
                confirm_label,
                confirm_yes,
                confirm_no,
                candidate: RefCell::new(None),
                applied: RefCell::new(None),
                applying: Cell::new(false),
                syncing: Cell::new(false),
            }),
        };
        generator.refresh_i18n();
        generator.update_bounds();
        generator.connect_signals(target);
        generator
    }

    fn slider_row(adjustment: &gtk4::Adjustment) -> gtk4::Box {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let scale = gtk4::Scale::new(gtk4::Orientation::Horizontal, Some(adjustment));
        scale.set_draw_value(false);
        scale.set_hexpand(true);
        let spin = gtk4::SpinButton::new(Some(adjustment), 1.0, 0);
        spin.set_valign(gtk4::Align::Center);
        row.append(&scale);
        row.append(&spin);
        row
    }

    fn connect_signals(&self, target: &gtk4::PasswordEntry) {
        let weak = Rc::downgrade(&self.inner);
        let on_change = move || {
            if let Some(inner) = weak.upgrade() {
                let generator = Self { inner };
                if generator.inner.syncing.get() {
                    return;
                }
                generator.update_bounds();
                generator.regenerate_if_open();
            }
        };
        let on_change = Rc::new(on_change);
        for adjustment in [&self.inner.length, &self.inner.words] {
            let on_change = Rc::clone(&on_change);
            adjustment.connect_value_changed(move |_| on_change());
        }
        for check in [
            &self.inner.lowercase,
            &self.inner.uppercase,
            &self.inner.digits,
            &self.inner.symbols,
            &self.inner.exclude_ambiguous,
        ] {
            let on_change = Rc::clone(&on_change);
            check.connect_toggled(move |_| on_change());
        }
        for dropdown in [
            &self.inner.language,
            &self.inner.separator,
            &self.inner.capitalization,
        ] {
            let on_change = Rc::clone(&on_change);
            dropdown.connect_selected_notify(move |_| on_change());
        }
        for spin in [&self.inner.extra_digits, &self.inner.extra_symbols] {
            let on_change = Rc::clone(&on_change);
            spin.connect_value_changed(move |_| on_change());
        }

        let weak = Rc::downgrade(&self.inner);
        self.inner.regenerate.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                Self { inner }.regenerate();
            }
        });
        let weak = Rc::downgrade(&self.inner);
        self.inner.use_button.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                Self { inner }.request_use();
            }
        });
        let weak = Rc::downgrade(&self.inner);
        self.inner.confirm_yes.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                Self { inner }.apply_candidate();
            }
        });
        let weak = Rc::downgrade(&self.inner);
        self.inner.confirm_no.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.confirm.set_reveal_child(false);
            }
        });
        // A manual edit of the value invalidates the recipe of the last applied candidate.
        let weak = Rc::downgrade(&self.inner);
        target.connect_text_notify(move |_| {
            if let Some(inner) = weak.upgrade()
                && !inner.applying.get()
            {
                inner.applied.borrow_mut().take();
            }
        });
    }

    pub fn widget(&self) -> &gtk4::Revealer {
        &self.inner.revealer
    }

    pub fn is_open(&self) -> bool {
        self.inner.revealer.reveals_child()
    }

    /// Opens (with a fresh candidate) or closes (wiping the candidate) the panel.
    pub fn set_open(&self, open: bool) {
        self.inner.revealer.set_reveal_child(open);
        if open {
            self.regenerate();
        } else {
            self.clear_candidate();
        }
    }

    /// Switches between the password and passphrase generators.
    pub fn set_kind(&self, kind: PasswordKind) {
        if self.inner.kind.get() == kind {
            return;
        }
        self.inner.kind.set(kind);
        self.inner.options.set_visible_child_name(match kind {
            PasswordKind::Passphrase => "passphrase",
            _ => "password",
        });
        self.regenerate_if_open();
    }

    /// Loads the settings of a stored recipe (e.g. from the edited item).
    pub fn set_recipe(&self, recipe: &GeneratorRecipe) {
        if recipe.validate().is_err() {
            return;
        }
        self.inner.syncing.set(true);
        match recipe {
            GeneratorRecipe::Password(recipe) => {
                self.inner.lowercase.set_active(recipe.lowercase);
                self.inner.uppercase.set_active(recipe.uppercase);
                self.inner.digits.set_active(recipe.digits);
                self.inner.symbols.set_active(recipe.symbols);
                self.inner
                    .exclude_ambiguous
                    .set_active(recipe.exclude_ambiguous);
                self.update_bounds();
                self.inner.length.set_value(f64::from(recipe.length));
            }
            GeneratorRecipe::Passphrase(recipe) => {
                self.inner
                    .language
                    .set_selected(index_of(&LANGUAGES, &recipe.language));
                self.inner
                    .separator
                    .set_selected(index_of(&SEPARATORS, &recipe.separator));
                self.inner
                    .capitalization
                    .set_selected(index_of(&CAPITALIZATIONS, &recipe.capitalization));
                self.inner.extra_digits.set_value(f64::from(recipe.digits));
                self.inner
                    .extra_symbols
                    .set_value(f64::from(recipe.symbols));
                self.update_bounds();
                self.inner.words.set_value(f64::from(recipe.words));
            }
            _ => {}
        }
        self.inner.syncing.set(false);
        self.update_bounds();
        self.regenerate_if_open();
    }

    /// Recipe of the value currently in the entry, if it was produced by this generator and
    /// not edited since.
    pub fn applied_recipe(&self) -> Option<GeneratorRecipe> {
        self.inner.applied.borrow().clone()
    }

    /// Security clearing: wipes the candidate and forgets the applied recipe.
    pub fn wipe(&self) {
        self.clear_candidate();
        self.inner.applied.borrow_mut().take();
        self.inner.revealer.set_reveal_child(false);
    }

    fn password_recipe(&self) -> PasswordRecipe {
        let inner = &self.inner;
        let mut recipe = PasswordRecipe::default();
        recipe.length = u16::try_from(inner.length.value().round() as i64)
            .unwrap_or(STORED_PASSWORD_MIN_LENGTH);
        recipe.lowercase = inner.lowercase.is_active();
        recipe.uppercase = inner.uppercase.is_active();
        recipe.digits = inner.digits.is_active();
        recipe.symbols = inner.symbols.is_active();
        recipe.exclude_ambiguous = inner.exclude_ambiguous.is_active();
        recipe
    }

    fn passphrase_recipe(&self) -> PassphraseRecipe {
        let inner = &self.inner;
        let mut recipe = PassphraseRecipe::default();
        recipe.words =
            u8::try_from(inner.words.value().round() as i64).unwrap_or(MIN_PASSPHRASE_WORDS);
        recipe.language = value_at(&LANGUAGES, inner.language.selected());
        recipe.separator = value_at(&SEPARATORS, inner.separator.selected());
        recipe.capitalization = value_at(&CAPITALIZATIONS, inner.capitalization.selected());
        recipe.digits = u8::try_from(inner.extra_digits.value_as_int()).unwrap_or(0);
        recipe.symbols = u8::try_from(inner.extra_symbols.value_as_int()).unwrap_or(0);
        recipe
    }

    pub fn recipe(&self) -> GeneratorRecipe {
        match self.inner.kind.get() {
            PasswordKind::Passphrase => GeneratorRecipe::Passphrase(self.passphrase_recipe()),
            _ => GeneratorRecipe::Password(self.password_recipe()),
        }
    }

    /// Raises the slider minimums to the entropy floor of the current options.
    fn update_bounds(&self) {
        let inner = &self.inner;
        let was_syncing = inner.syncing.replace(true);
        let password_min = self
            .password_recipe()
            .min_length_for_floor()
            .map(|min| min.max(STORED_PASSWORD_MIN_LENGTH));
        if let Some(min) = password_min {
            inner.length.set_lower(f64::from(min));
            if inner.length.value() < f64::from(min) {
                inner.length.set_value(f64::from(min));
            }
            inner.length_hint.set_label(&tr_args(
                "generator-length-min-hint",
                &[("min", I18nArg::Num(i64::from(min)))],
            ));
        } else {
            inner
                .length_hint
                .set_label(&heelonvault_core::tr!("generator-no-class"));
        }
        if let Some(min) = self.passphrase_recipe().min_words_for_floor() {
            inner.words.set_lower(f64::from(min));
            if inner.words.value() < f64::from(min) {
                inner.words.set_value(f64::from(min));
            }
            inner.words_hint.set_label(&tr_args(
                "generator-words-min-hint",
                &[("min", I18nArg::Num(i64::from(min)))],
            ));
        }
        inner.syncing.set(was_syncing);
    }

    fn regenerate_if_open(&self) {
        if self.is_open() {
            self.regenerate();
        }
    }

    fn regenerate(&self) {
        let inner = &self.inner;
        inner.confirm.set_reveal_child(false);
        let recipe = self.recipe();
        let bits = recipe.entropy_bits();
        self.show_entropy(bits, recipe.level());
        match recipe.generate() {
            Ok(secret) => {
                let markup = preview_markup(secret.expose_secret());
                inner.preview.set_markup(&markup);
                *inner.candidate.borrow_mut() = Some(secret);
                inner.use_button.set_sensitive(true);
            }
            Err(_) => {
                self.clear_candidate();
                inner.use_button.set_sensitive(false);
            }
        }
    }

    fn show_entropy(&self, bits: f64, level: EntropyLevel) {
        let entropy = &self.inner.entropy;
        let level_text = match level {
            EntropyLevel::Strong => heelonvault_core::tr!("generator-level-strong"),
            EntropyLevel::Medium => heelonvault_core::tr!("generator-level-medium"),
            EntropyLevel::Weak => heelonvault_core::tr!("generator-level-weak"),
            _ => heelonvault_core::tr!("generator-level-very-weak"),
        };
        entropy.set_label(&tr_args(
            "generator-entropy",
            &[
                ("bits", I18nArg::Num(bits.floor() as i64)),
                ("level", I18nArg::Str(&level_text)),
            ],
        ));
        for class in ["success", "warning", "error"] {
            entropy.remove_css_class(class);
        }
        entropy.add_css_class(match level {
            EntropyLevel::Strong => "success",
            EntropyLevel::Medium => "warning",
            _ => "error",
        });
    }

    fn clear_candidate(&self) {
        let inner = &self.inner;
        inner.candidate.borrow_mut().take();
        inner.preview.set_label("");
        inner.confirm.set_reveal_child(false);
    }

    fn request_use(&self) {
        let inner = &self.inner;
        let Some(target) = inner.target.upgrade() else {
            return;
        };
        let differs = {
            let candidate = inner.candidate.borrow();
            let Some(candidate) = candidate.as_ref() else {
                return;
            };
            let current = target.text();
            !current.is_empty() && current.as_bytes() != candidate.expose_secret().as_slice()
        };
        if differs {
            inner.confirm.set_reveal_child(true);
        } else {
            self.apply_candidate();
        }
    }

    fn apply_candidate(&self) {
        let inner = &self.inner;
        let Some(target) = inner.target.upgrade() else {
            return;
        };
        let Some(candidate) = inner.candidate.borrow_mut().take() else {
            return;
        };
        let Ok(text) = std::str::from_utf8(candidate.expose_secret()) else {
            return;
        };
        let text = Zeroizing::new(text.to_string());
        inner.applying.set(true);
        target.set_text(&text);
        inner.applying.set(false);
        drop(text);
        drop(candidate);
        *inner.applied.borrow_mut() = Some(self.recipe());
        if !cfg!(test) {
            view_preferences::persist_generator_recipes(
                &self.password_recipe(),
                &self.passphrase_recipe(),
            );
        }
        self.set_open(false);
    }

    pub fn refresh_i18n(&self) {
        let inner = &self.inner;
        inner
            .regenerate
            .set_label(&heelonvault_core::tr!("generator-regenerate"));
        inner
            .use_button
            .set_label(&heelonvault_core::tr!("generator-use"));
        inner
            .length_title
            .set_label(&heelonvault_core::tr!("generator-length"));
        inner
            .lowercase
            .set_label(Some(&heelonvault_core::tr!("generator-class-lower")));
        inner
            .uppercase
            .set_label(Some(&heelonvault_core::tr!("generator-class-upper")));
        inner
            .digits
            .set_label(Some(&heelonvault_core::tr!("generator-class-digits")));
        inner
            .symbols
            .set_label(Some(&heelonvault_core::tr!("generator-class-symbols")));
        inner
            .exclude_ambiguous
            .set_label(Some(&heelonvault_core::tr!("generator-exclude-ambiguous")));
        inner
            .words_title
            .set_label(&heelonvault_core::tr!("generator-words"));
        inner
            .language_title
            .set_label(&heelonvault_core::tr!("generator-language"));
        inner
            .separator_title
            .set_label(&heelonvault_core::tr!("generator-separator"));
        inner
            .capitalization_title
            .set_label(&heelonvault_core::tr!("generator-capitalization"));
        inner
            .extra_digits_title
            .set_label(&heelonvault_core::tr!("generator-extra-digits"));
        inner
            .extra_symbols_title
            .set_label(&heelonvault_core::tr!("generator-extra-symbols"));
        inner
            .confirm_label
            .set_label(&heelonvault_core::tr!("generator-replace-question"));
        inner
            .confirm_yes
            .set_label(&heelonvault_core::tr!("generator-replace-confirm"));
        inner
            .confirm_no
            .set_label(&heelonvault_core::tr!("generator-replace-cancel"));
        inner.syncing.set(true);
        set_strings(
            &inner.language,
            &[
                heelonvault_core::tr!("generator-language-en"),
                heelonvault_core::tr!("generator-language-fr"),
            ],
        );
        set_strings(
            &inner.separator,
            &[
                heelonvault_core::tr!("generator-separator-hyphen"),
                heelonvault_core::tr!("generator-separator-dot"),
                heelonvault_core::tr!("generator-separator-underscore"),
                heelonvault_core::tr!("generator-separator-space"),
                heelonvault_core::tr!("generator-separator-random"),
            ],
        );
        set_strings(
            &inner.capitalization,
            &[
                heelonvault_core::tr!("generator-capitalization-none"),
                heelonvault_core::tr!("generator-capitalization-first"),
                heelonvault_core::tr!("generator-capitalization-random"),
            ],
        );
        inner.syncing.set(false);
        self.update_bounds();
        let recipe = self.recipe();
        self.show_entropy(recipe.entropy_bits(), recipe.level());
    }
}

#[cfg(test)]
impl PasswordGenerator {
    /// Clicks "Use" as a user would.
    pub(crate) fn click_use(&self) {
        self.inner.use_button.emit_clicked();
    }

    pub(crate) fn confirmation_pending(&self) -> bool {
        self.inner.confirm.reveals_child()
    }

    pub(crate) fn click_confirm(&self) {
        self.inner.confirm_yes.emit_clicked();
    }

    pub(crate) fn settings_widget(&self) -> gtk4::CheckButton {
        self.inner.symbols.clone()
    }
}

#[cfg(test)]
pub(crate) fn native_generator_scenario() -> Result<(), Box<dyn std::error::Error>> {
    use heelonvault_core::services::password_generator::MIN_GENERATED_ENTROPY_BITS;

    let entry = gtk4::PasswordEntry::new();
    let generator = PasswordGenerator::new(&entry);
    let inner = &generator.inner;
    let mut password = PasswordRecipe::default();
    password.length = 20;
    generator.set_recipe(&GeneratorRecipe::Password(password.clone()));

    // Opening shows a candidate; "Use" fills an empty entry directly and closes the panel.
    generator.set_open(true);
    assert!(!inner.preview.label().is_empty());
    assert!(inner.use_button.is_sensitive());
    assert!(inner.entropy.has_css_class("success"));
    generator.click_use();
    assert_eq!(entry.text().chars().count(), 20);
    assert!(!generator.is_open());
    assert!(
        inner.preview.label().is_empty(),
        "candidate wiped after use"
    );
    assert_eq!(
        generator.applied_recipe(),
        Some(GeneratorRecipe::Password(password))
    );

    // A manual edit drops the recipe; replacing a different value needs a confirmation.
    entry.set_text("typed by hand");
    assert_eq!(generator.applied_recipe(), None);
    generator.set_kind(PasswordKind::Passphrase);
    assert_eq!(
        inner.options.visible_child_name().as_deref(),
        Some("passphrase")
    );
    generator.set_open(true);
    generator.click_use();
    assert!(generator.confirmation_pending());
    assert_eq!(entry.text().as_str(), "typed by hand");
    generator.click_confirm();
    let words = entry
        .text()
        .split(['-', '.', '_', ' '])
        .filter(|part| part.chars().any(char::is_alphabetic))
        .count();
    assert!(words >= 7, "{words} words");
    assert!(matches!(
        generator.applied_recipe(),
        Some(GeneratorRecipe::Passphrase(_))
    ));

    // Slider minimums follow the 80-bit floor of the selected options.
    generator.set_kind(PasswordKind::Password);
    for check in [&inner.lowercase, &inner.uppercase, &inner.symbols] {
        check.set_active(false);
    }
    let digits_only = generator.password_recipe();
    assert!(digits_only.entropy_bits() >= MIN_GENERATED_ENTROPY_BITS);
    assert!(inner.length.lower() >= 27.0);
    inner.digits.set_active(false);
    generator.set_open(true);
    assert!(!inner.use_button.is_sensitive(), "no class enabled");
    inner.digits.set_active(true);
    assert!(inner.use_button.is_sensitive());

    // Live retranslation keeps the selections.
    inner.separator.set_selected(3);
    assert!(heelonvault_core::i18n::set_language("fr"));
    generator.refresh_i18n();
    assert_eq!(
        inner.use_button.label().as_deref(),
        Some(heelonvault_core::tr!("generator-use").as_str())
    );
    assert_eq!(inner.separator.selected(), 3);
    assert!(heelonvault_core::i18n::set_language("en"));
    generator.refresh_i18n();

    // Security clearing wipes everything.
    generator.wipe();
    assert!(!generator.is_open());
    assert!(inner.preview.label().is_empty());
    assert_eq!(generator.applied_recipe(), None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::preview_markup;

    #[test]
    fn preview_markup_escapes_and_colors() {
        let markup = preview_markup(b"a<1 &");
        assert!(markup.contains("&lt;"));
        assert!(markup.contains("&amp;"));
        assert!(markup.contains("<span foreground=\"#3584e4\">1</span>"));
        assert!(markup.contains('\u{2423}'));
        assert!(!markup.contains("<1"));
    }
}

//! Configurable password and passphrase generation with an entropy-based policy.
//!
//! The policy is expressed in entropy bits rather than characters: a recipe below
//! [`MIN_GENERATED_ENTROPY_BITS`] (CNIL deliberation 2022-100 floor) is refused, and the
//! defaults target at least [`STRONG_ENTROPY_BITS`] (ANSSI "strong" band). Entropy is a
//! conservative lower bound: only uniformly random choices are counted.
//!
//! A recipe is serializable and never contains secret material, so it can be stored in the
//! item metadata to regenerate a value with the same settings.

use std::sync::OnceLock;

use getrandom::fill;
use secrecy::{ExposeSecretMut, SecretBox};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::errors::AppError;

/// Generated values below this strength are refused (CNIL floor for a password used alone).
pub const MIN_GENERATED_ENTROPY_BITS: f64 = 80.0;
/// ANSSI "strong" band; the default recipes reach it.
pub const STRONG_ENTROPY_BITS: f64 = 100.0;

/// Structural bounds of a password recipe; the entropy floor usually raises the minimum.
pub const MIN_PASSWORD_RECIPE_LENGTH: u16 = 8;
pub const MAX_PASSWORD_RECIPE_LENGTH: u16 = 128;
pub const DEFAULT_PASSWORD_RECIPE_LENGTH: u16 = 20;

/// Structural bounds of a passphrase recipe; the entropy floor usually raises the minimum.
pub const MIN_PASSPHRASE_WORDS: u8 = 3;
pub const MAX_PASSPHRASE_WORDS: u8 = 20;
pub const DEFAULT_PASSPHRASE_WORDS: u8 = 8;
/// Maximum number of extra digits, and separately of extra symbols, in a passphrase.
pub const MAX_PASSPHRASE_EXTRAS: u8 = 3;

const LOWERCASE_UNAMBIGUOUS: &[u8] = b"abcdefghijkmnopqrstuvwxyz";
const UPPERCASE_UNAMBIGUOUS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITS_UNAMBIGUOUS: &[u8] = b"23456789";
const LOWERCASE_ALL: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPERCASE_ALL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS_ALL: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{}:;,.?/";

/// Upper bound of rejection rounds for a password; reaching it is practically impossible
/// (every valid recipe has an acceptance probability far above 1 %).
const MAX_PASSWORD_ATTEMPTS: usize = 10_000;

/// ANSSI strength bands for an entropy value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntropyLevel {
    /// Below 64 bits.
    VeryWeak,
    /// 64 to 80 bits.
    Weak,
    /// 80 to 100 bits.
    Medium,
    /// 100 bits and above.
    Strong,
}

impl EntropyLevel {
    pub fn from_bits(bits: f64) -> Self {
        if bits >= STRONG_ENTROPY_BITS {
            Self::Strong
        } else if bits >= MIN_GENERATED_ENTROPY_BITS {
            Self::Medium
        } else if bits >= 64.0 {
            Self::Weak
        } else {
            Self::VeryWeak
        }
    }
}

/// Word list used for passphrases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WordListLanguage {
    #[default]
    English,
    /// French BIP39 list without accented words (easy to type on any keyboard).
    French,
}

impl WordListLanguage {
    /// Words available for this language (lowercase ASCII only, prefix-unique BIP39 words).
    pub fn words(self) -> &'static [&'static str] {
        static ENGLISH: OnceLock<Vec<&'static str>> = OnceLock::new();
        static FRENCH: OnceLock<Vec<&'static str>> = OnceLock::new();
        let (cell, language) = match self {
            Self::English => (&ENGLISH, bip39::Language::English),
            Self::French => (&FRENCH, bip39::Language::French),
        };
        cell.get_or_init(|| {
            language
                .word_list()
                .iter()
                .copied()
                .filter(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase()))
                .collect()
        })
        .as_slice()
    }
}

/// Separator placed between passphrase words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PassphraseSeparator {
    #[default]
    Hyphen,
    Dot,
    Underscore,
    Space,
    /// A random digit or symbol for each gap (adds entropy, harder to type).
    RandomDigitOrSymbol,
}

impl PassphraseSeparator {
    fn fixed_byte(self) -> Option<u8> {
        match self {
            Self::Hyphen => Some(b'-'),
            Self::Dot => Some(b'.'),
            Self::Underscore => Some(b'_'),
            Self::Space => Some(b' '),
            Self::RandomDigitOrSymbol => None,
        }
    }
}

/// Capitalization applied to passphrase words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PassphraseCapitalization {
    /// All words in lowercase.
    None,
    /// Every word starts with an uppercase letter (no added entropy).
    FirstLetter,
    /// Each word independently starts with an uppercase letter or not (one bit per word).
    #[default]
    Random,
}

/// Settings of a random character password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct PasswordRecipe {
    pub length: u16,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
    /// Excludes look-alike characters (`l`, `I`, `O`, `0`, `1`).
    pub exclude_ambiguous: bool,
}

impl Default for PasswordRecipe {
    fn default() -> Self {
        Self {
            length: DEFAULT_PASSWORD_RECIPE_LENGTH,
            lowercase: true,
            uppercase: true,
            digits: true,
            symbols: true,
            exclude_ambiguous: true,
        }
    }
}

impl PasswordRecipe {
    fn classes(&self) -> Vec<&'static [u8]> {
        let mut classes: Vec<&'static [u8]> = Vec::with_capacity(4);
        if self.lowercase {
            classes.push(if self.exclude_ambiguous {
                LOWERCASE_UNAMBIGUOUS
            } else {
                LOWERCASE_ALL
            });
        }
        if self.uppercase {
            classes.push(if self.exclude_ambiguous {
                UPPERCASE_UNAMBIGUOUS
            } else {
                UPPERCASE_ALL
            });
        }
        if self.digits {
            classes.push(if self.exclude_ambiguous {
                DIGITS_UNAMBIGUOUS
            } else {
                DIGITS_ALL
            });
        }
        if self.symbols {
            classes.push(SYMBOLS);
        }
        classes
    }

    /// Exact entropy of the generated distribution: uniform over every string of `length`
    /// characters that contains at least one character of each enabled class.
    pub fn entropy_bits(&self) -> f64 {
        Self::entropy_for(&self.classes(), self.length)
    }

    fn entropy_for(classes: &[&[u8]], length: u16) -> f64 {
        let total: usize = classes.iter().map(|class| class.len()).sum();
        if total == 0 || usize::from(length) < classes.len() {
            return 0.0;
        }
        let total_f = total as f64;
        let exponent = i32::from(length);
        // Inclusion–exclusion over the classes that may be missing, as a ratio of total^length.
        let mut ratio = 0.0_f64;
        for mask in 0_u32..(1_u32 << classes.len()) {
            let missing: usize = classes
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, class)| class.len())
                .sum();
            let term = ((total - missing) as f64 / total_f).powi(exponent);
            if mask.count_ones() % 2 == 0 {
                ratio += term;
            } else {
                ratio -= term;
            }
        }
        if ratio <= 0.0 {
            return 0.0;
        }
        f64::from(length) * total_f.log2() + ratio.log2()
    }

    /// Smallest length reaching the entropy floor with the current classes, if any.
    pub fn min_length_for_floor(&self) -> Option<u16> {
        let classes = self.classes();
        (MIN_PASSWORD_RECIPE_LENGTH..=MAX_PASSWORD_RECIPE_LENGTH)
            .find(|length| Self::entropy_for(&classes, *length) >= MIN_GENERATED_ENTROPY_BITS)
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if !(MIN_PASSWORD_RECIPE_LENGTH..=MAX_PASSWORD_RECIPE_LENGTH).contains(&self.length) {
            return Err(AppError::Validation(format!(
                "password length must be between {MIN_PASSWORD_RECIPE_LENGTH} and {MAX_PASSWORD_RECIPE_LENGTH} characters"
            )));
        }
        if self.classes().is_empty() {
            return Err(AppError::Validation(
                "at least one character class must be enabled".to_string(),
            ));
        }
        ensure_floor(self.entropy_bits())
    }

    fn generate(&self) -> Result<SecretBox<Vec<u8>>, AppError> {
        self.validate()?;
        let classes = self.classes();
        let combined: Vec<u8> = classes
            .iter()
            .flat_map(|class| class.iter().copied())
            .collect();
        let length = usize::from(self.length);
        let mut buffer = SecretBox::new(Box::new(Vec::with_capacity(length)));
        // Rejection sampling keeps the output uniform over valid strings; a rejected candidate
        // is overwritten in place by the next one.
        for _ in 0..MAX_PASSWORD_ATTEMPTS {
            let candidate = buffer.expose_secret_mut();
            candidate.clear();
            for _ in 0..length {
                candidate.push(sample(&combined)?);
            }
            let complete = classes
                .iter()
                .all(|class| candidate.iter().any(|byte| class.contains(byte)));
            if complete {
                return Ok(buffer);
            }
        }
        Err(AppError::Crypto(
            "password generation did not converge".to_string(),
        ))
    }
}

/// Settings of a word-based passphrase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct PassphraseRecipe {
    pub words: u8,
    pub language: WordListLanguage,
    pub separator: PassphraseSeparator,
    pub capitalization: PassphraseCapitalization,
    /// Random digits appended to random words.
    pub digits: u8,
    /// Random symbols appended to random words.
    pub symbols: u8,
}

impl Default for PassphraseRecipe {
    fn default() -> Self {
        Self {
            words: DEFAULT_PASSPHRASE_WORDS,
            language: WordListLanguage::default(),
            separator: PassphraseSeparator::default(),
            capitalization: PassphraseCapitalization::default(),
            digits: 1,
            symbols: 1,
        }
    }
}

impl PassphraseRecipe {
    /// Conservative entropy: word choices, random capitalization, random separators and the
    /// values of the extra characters. Extra positions are not counted.
    pub fn entropy_bits(&self) -> f64 {
        Self::entropy_for(self, self.words)
    }

    fn entropy_for(recipe: &Self, words: u8) -> f64 {
        let list_len = recipe.language.words().len();
        if words == 0 || list_len == 0 {
            return 0.0;
        }
        let words_f = f64::from(words);
        let mut bits = words_f * (list_len as f64).log2();
        if recipe.capitalization == PassphraseCapitalization::Random {
            bits += words_f;
        }
        if recipe.separator == PassphraseSeparator::RandomDigitOrSymbol {
            bits += f64::from(words - 1) * ((DIGITS_ALL.len() + SYMBOLS.len()) as f64).log2();
        }
        bits += f64::from(recipe.digits) * (DIGITS_ALL.len() as f64).log2();
        bits += f64::from(recipe.symbols) * (SYMBOLS.len() as f64).log2();
        bits
    }

    /// Smallest word count reaching the entropy floor with the current settings, if any.
    pub fn min_words_for_floor(&self) -> Option<u8> {
        (MIN_PASSPHRASE_WORDS..=MAX_PASSPHRASE_WORDS)
            .find(|words| Self::entropy_for(self, *words) >= MIN_GENERATED_ENTROPY_BITS)
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if !(MIN_PASSPHRASE_WORDS..=MAX_PASSPHRASE_WORDS).contains(&self.words) {
            return Err(AppError::Validation(format!(
                "passphrase must contain between {MIN_PASSPHRASE_WORDS} and {MAX_PASSPHRASE_WORDS} words"
            )));
        }
        if self.digits > MAX_PASSPHRASE_EXTRAS || self.symbols > MAX_PASSPHRASE_EXTRAS {
            return Err(AppError::Validation(format!(
                "passphrase extras must not exceed {MAX_PASSPHRASE_EXTRAS} digits and {MAX_PASSPHRASE_EXTRAS} symbols"
            )));
        }
        ensure_floor(self.entropy_bits())
    }

    fn generate(&self) -> Result<SecretBox<Vec<u8>>, AppError> {
        self.validate()?;
        let list = self.language.words();
        let words = usize::from(self.words);

        // Every random choice is drawn first, held in zeroized buffers, then assembled once
        // into a buffer allocated at its final size (no reallocation copies of the secret).
        let mut word_indices = Zeroizing::new(Vec::with_capacity(words));
        let mut capitalized = Zeroizing::new(Vec::with_capacity(words));
        for _ in 0..words {
            word_indices.push(random_index(list.len())?);
            capitalized.push(match self.capitalization {
                PassphraseCapitalization::None => false,
                PassphraseCapitalization::FirstLetter => true,
                PassphraseCapitalization::Random => random_index(2)? == 1,
            });
        }
        let separator_pool: Vec<u8> = DIGITS_ALL.iter().chain(SYMBOLS).copied().collect();
        let mut separators = Zeroizing::new(Vec::with_capacity(words.saturating_sub(1)));
        for _ in 1..words {
            separators.push(match self.separator.fixed_byte() {
                Some(byte) => byte,
                None => sample(&separator_pool)?,
            });
        }
        // (word index, extra byte), in generation order.
        let extra_count = usize::from(self.digits) + usize::from(self.symbols);
        let mut extras = Zeroizing::new(Vec::with_capacity(extra_count));
        for _ in 0..self.digits {
            extras.push((random_index(words)?, sample(DIGITS_ALL)?));
        }
        for _ in 0..self.symbols {
            extras.push((random_index(words)?, sample(SYMBOLS)?));
        }

        let mut total = separators.len() + extras.len();
        for index in word_indices.iter() {
            total += list.get(*index).map_or(0, |word| word.len());
        }
        let mut output = Vec::with_capacity(total);
        for (position, index) in word_indices.iter().enumerate() {
            let word = list
                .get(*index)
                .ok_or_else(|| AppError::Crypto("word index out of range".to_string()))?;
            for (offset, byte) in word.bytes().enumerate() {
                let upper = offset == 0 && capitalized.get(position).copied().unwrap_or(false);
                output.push(if upper {
                    byte.to_ascii_uppercase()
                } else {
                    byte
                });
            }
            for (target, byte) in extras.iter() {
                if *target == position {
                    output.push(*byte);
                }
            }
            if let Some(separator) = separators.get(position) {
                output.push(*separator);
            }
        }
        Ok(SecretBox::new(Box::new(output)))
    }
}

/// A generation recipe: either kind of generated secret, with its settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum GeneratorRecipe {
    Password(PasswordRecipe),
    Passphrase(PassphraseRecipe),
}

impl Default for GeneratorRecipe {
    fn default() -> Self {
        Self::Password(PasswordRecipe::default())
    }
}

impl GeneratorRecipe {
    pub fn entropy_bits(&self) -> f64 {
        match self {
            Self::Password(recipe) => recipe.entropy_bits(),
            Self::Passphrase(recipe) => recipe.entropy_bits(),
        }
    }

    pub fn level(&self) -> EntropyLevel {
        EntropyLevel::from_bits(self.entropy_bits())
    }

    pub fn validate(&self) -> Result<(), AppError> {
        match self {
            Self::Password(recipe) => recipe.validate(),
            Self::Passphrase(recipe) => recipe.validate(),
        }
    }

    /// Generates a value from the OS CSPRNG; refuses recipes below the entropy floor.
    pub fn generate(&self) -> Result<SecretBox<Vec<u8>>, AppError> {
        match self {
            Self::Password(recipe) => recipe.generate(),
            Self::Passphrase(recipe) => recipe.generate(),
        }
    }
}

fn ensure_floor(bits: f64) -> Result<(), AppError> {
    if bits < MIN_GENERATED_ENTROPY_BITS {
        return Err(AppError::Validation(format!(
            "generated secret must reach at least {MIN_GENERATED_ENTROPY_BITS} bits of entropy"
        )));
    }
    Ok(())
}

fn sample(charset: &[u8]) -> Result<u8, AppError> {
    let index = random_index(charset.len())?;
    charset
        .get(index)
        .copied()
        .ok_or_else(|| AppError::Crypto("random index out of range".to_string()))
}

/// Unbiased index in `0..bound` from the OS CSPRNG (rejection sampling).
pub(crate) fn random_index(bound: usize) -> Result<usize, AppError> {
    if bound == 0 {
        return Err(AppError::Validation(
            "random selection bound must be greater than zero".to_string(),
        ));
    }
    let bound_u32 = u32::try_from(bound).map_err(|_| {
        AppError::Validation("random selection bound exceeds supported size".to_string())
    })?;
    let zone = u32::MAX - (u32::MAX % bound_u32);
    loop {
        let mut random_bytes = Zeroizing::new([0_u8; 4]);
        fill(random_bytes.as_mut())
            .map_err(|err| AppError::Crypto(format!("random generation failed: {err}")))?;
        let candidate = u32::from_le_bytes(*random_bytes);
        if candidate < zone {
            return usize::try_from(candidate % bound_u32)
                .map_err(|_| AppError::Validation("random index conversion failed".to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 0.05
    }

    #[test]
    fn french_list_is_ascii_lowercase_and_large() {
        let words = WordListLanguage::French.words();
        assert_eq!(words.len(), 1682);
        assert!(
            words
                .iter()
                .all(|word| word.bytes().all(|b| b.is_ascii_lowercase()))
        );
        assert_eq!(WordListLanguage::English.words().len(), 2048);
    }

    #[test]
    fn default_recipes_are_strong() {
        let password = GeneratorRecipe::default();
        assert_eq!(password.level(), EntropyLevel::Strong);
        for language in [WordListLanguage::English, WordListLanguage::French] {
            let recipe = PassphraseRecipe {
                language,
                ..PassphraseRecipe::default()
            };
            assert!(recipe.entropy_bits() >= STRONG_ENTROPY_BITS, "{language:?}");
        }
    }

    #[test]
    fn password_entropy_matches_closed_forms() {
        // A single class has no completeness constraint: length × log2(size).
        let lowercase_only = PasswordRecipe {
            length: 20,
            uppercase: false,
            digits: false,
            symbols: false,
            ..PasswordRecipe::default()
        };
        assert!(close(lowercase_only.entropy_bits(), 20.0 * 25_f64.log2()));
        // With all classes, the constraint costs a little below length × log2(81).
        let all = PasswordRecipe::default();
        let upper_bound = 20.0 * 81_f64.log2();
        assert!(all.entropy_bits() < upper_bound);
        assert!(all.entropy_bits() > upper_bound - 2.0);
    }

    #[test]
    fn password_floor_is_dynamic() {
        assert_eq!(PasswordRecipe::default().min_length_for_floor(), Some(13));
        let digits_only = PasswordRecipe {
            lowercase: false,
            uppercase: false,
            symbols: false,
            exclude_ambiguous: false,
            ..PasswordRecipe::default()
        };
        assert_eq!(digits_only.min_length_for_floor(), Some(25));
        let short = PasswordRecipe {
            length: 12,
            ..PasswordRecipe::default()
        };
        assert!(matches!(short.validate(), Err(AppError::Validation(_))));
        let none = PasswordRecipe {
            lowercase: false,
            uppercase: false,
            digits: false,
            symbols: false,
            ..PasswordRecipe::default()
        };
        assert!(none.validate().is_err());
        assert_eq!(none.min_length_for_floor(), None);
    }

    #[test]
    fn generated_password_honours_length_and_classes() -> Result<(), AppError> {
        for exclude_ambiguous in [true, false] {
            let recipe = PasswordRecipe {
                length: 16,
                exclude_ambiguous,
                ..PasswordRecipe::default()
            };
            for _ in 0..200 {
                let secret = GeneratorRecipe::Password(recipe.clone()).generate()?;
                let value = secret.expose_secret();
                assert_eq!(value.len(), 16);
                assert!(value.iter().any(u8::is_ascii_lowercase));
                assert!(value.iter().any(u8::is_ascii_uppercase));
                assert!(value.iter().any(u8::is_ascii_digit));
                assert!(value.iter().any(|byte| SYMBOLS.contains(byte)));
                if exclude_ambiguous {
                    assert!(!value.iter().any(|byte| b"lIO01".contains(byte)));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn disabled_classes_never_appear() -> Result<(), AppError> {
        let recipe = PasswordRecipe {
            length: 24,
            symbols: false,
            ..PasswordRecipe::default()
        };
        for _ in 0..100 {
            let secret = GeneratorRecipe::Password(recipe.clone()).generate()?;
            assert!(secret.expose_secret().iter().all(u8::is_ascii_alphanumeric));
        }
        Ok(())
    }

    #[test]
    fn passphrase_entropy_formula() {
        let recipe = PassphraseRecipe {
            words: 8,
            language: WordListLanguage::French,
            ..PassphraseRecipe::default()
        };
        let expected = 8.0 * 1682_f64.log2() + 8.0 + 10_f64.log2() + 24_f64.log2();
        assert!(close(recipe.entropy_bits(), expected));
        let random_separator = PassphraseRecipe {
            separator: PassphraseSeparator::RandomDigitOrSymbol,
            capitalization: PassphraseCapitalization::FirstLetter,
            digits: 0,
            symbols: 0,
            ..recipe
        };
        let expected = 8.0 * 1682_f64.log2() + 7.0 * 34_f64.log2();
        assert!(close(random_separator.entropy_bits(), expected));
    }

    #[test]
    fn passphrase_floor_is_dynamic() {
        assert_eq!(PassphraseRecipe::default().min_words_for_floor(), Some(7));
        let plain = PassphraseRecipe {
            capitalization: PassphraseCapitalization::None,
            digits: 0,
            symbols: 0,
            ..PassphraseRecipe::default()
        };
        assert_eq!(plain.min_words_for_floor(), Some(8));
        let six = PassphraseRecipe {
            words: 6,
            ..PassphraseRecipe::default()
        };
        assert!(six.validate().is_err());
        let too_many_extras = PassphraseRecipe {
            digits: 4,
            ..PassphraseRecipe::default()
        };
        assert!(too_many_extras.validate().is_err());
    }

    #[test]
    fn generated_passphrase_is_well_formed() -> Result<(), AppError> {
        let recipe = PassphraseRecipe {
            words: 7,
            language: WordListLanguage::French,
            capitalization: PassphraseCapitalization::FirstLetter,
            digits: 2,
            symbols: 1,
            ..PassphraseRecipe::default()
        };
        let list = WordListLanguage::French.words();
        for _ in 0..100 {
            let secret = GeneratorRecipe::Passphrase(recipe.clone()).generate()?;
            let value = std::str::from_utf8(secret.expose_secret())
                .map_err(|err| AppError::Validation(err.to_string()))?;
            let parts: Vec<&str> = value.split('-').filter(|part| !part.is_empty()).collect();
            let words: Vec<String> = parts
                .iter()
                .map(|part| {
                    part.trim_end_matches(|c: char| !c.is_ascii_alphabetic())
                        .to_ascii_lowercase()
                })
                .collect();
            let real_words: Vec<&String> = words.iter().filter(|word| !word.is_empty()).collect();
            assert_eq!(real_words.len(), 7);
            assert!(real_words.iter().all(|word| list.contains(&word.as_str())));
            assert!(
                parts
                    .iter()
                    .all(|part| part.chars().next().is_none_or(|c| !c.is_ascii_lowercase()))
            );
            assert_eq!(value.chars().filter(char::is_ascii_digit).count(), 2);
        }
        Ok(())
    }

    #[test]
    fn space_and_random_separators() -> Result<(), AppError> {
        let spaced = PassphraseRecipe {
            separator: PassphraseSeparator::Space,
            capitalization: PassphraseCapitalization::None,
            digits: 0,
            symbols: 0,
            words: 9,
            ..PassphraseRecipe::default()
        };
        let secret = GeneratorRecipe::Passphrase(spaced).generate()?;
        assert_eq!(
            secret
                .expose_secret()
                .iter()
                .filter(|byte| **byte == b' ')
                .count(),
            8
        );
        let random = PassphraseRecipe {
            separator: PassphraseSeparator::RandomDigitOrSymbol,
            words: 6,
            ..PassphraseRecipe::default()
        };
        let secret = GeneratorRecipe::Passphrase(random).generate()?;
        assert!(
            secret
                .expose_secret()
                .iter()
                .all(|byte| byte.is_ascii_graphic())
        );
        Ok(())
    }

    #[test]
    fn recipe_round_trips_through_json() -> Result<(), serde_json::Error> {
        let recipe = GeneratorRecipe::Passphrase(PassphraseRecipe {
            language: WordListLanguage::French,
            separator: PassphraseSeparator::Dot,
            ..PassphraseRecipe::default()
        });
        let json = serde_json::to_string(&recipe)?;
        assert!(json.contains("\"kind\":\"passphrase\""));
        assert!(json.contains("\"language\":\"french\""));
        assert_eq!(serde_json::from_str::<GeneratorRecipe>(&json)?, recipe);
        // Missing fields fall back to defaults (forward compatibility).
        let partial: GeneratorRecipe = serde_json::from_str(r#"{"kind":"password","length":32}"#)?;
        let expected = GeneratorRecipe::Password(PasswordRecipe {
            length: 32,
            ..PasswordRecipe::default()
        });
        assert_eq!(partial, expected);
        Ok(())
    }

    #[test]
    fn entropy_levels_follow_anssi_bands() {
        assert_eq!(EntropyLevel::from_bits(40.0), EntropyLevel::VeryWeak);
        assert_eq!(EntropyLevel::from_bits(64.0), EntropyLevel::Weak);
        assert_eq!(EntropyLevel::from_bits(80.0), EntropyLevel::Medium);
        assert_eq!(EntropyLevel::from_bits(100.0), EntropyLevel::Strong);
    }

    #[test]
    fn random_index_is_roughly_uniform() -> Result<(), AppError> {
        let mut counts = [0_u32; 6];
        for _ in 0..6000 {
            let index = random_index(6)?;
            if let Some(count) = counts.get_mut(index) {
                *count += 1;
            }
        }
        assert!(
            counts.iter().all(|count| (800..1200).contains(count)),
            "{counts:?}"
        );
        assert!(random_index(0).is_err());
        Ok(())
    }
}

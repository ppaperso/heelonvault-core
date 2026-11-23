use secrecy::{ExposeSecret, SecretBox};

use crate::errors::AppError;
use crate::services::password_generator::{GeneratorRecipe, PasswordRecipe};

const MIN_PASSWORD_LENGTH: usize = 16;
const MAX_PASSWORD_LENGTH: usize = 128;
const DEFAULT_GENERATED_PASSWORD_LENGTH: usize = 24;
const PASSWORD_STRENGTH_MAX: u8 = 100;

#[trait_variant::make(PasswordService: Send)]
pub trait LocalPasswordService {
    fn validate_password_policy(&self, password: &SecretBox<Vec<u8>>) -> Result<(), AppError>;
    fn score_password_strength(&self, password: &SecretBox<Vec<u8>>) -> Result<u8, AppError>;
    fn generate_password(&self, length: usize) -> Result<SecretBox<Vec<u8>>, AppError>;

    /// Generates a password or passphrase from a recipe; refuses recipes below the
    /// entropy floor (`password_generator::MIN_GENERATED_ENTROPY_BITS`).
    fn generate_from_recipe(
        &self,
        recipe: &GeneratorRecipe,
    ) -> Result<SecretBox<Vec<u8>>, AppError> {
        recipe.generate()
    }
}

pub struct PasswordServiceImpl;

impl PasswordServiceImpl {
    pub fn new() -> Self {
        Self
    }

    pub fn with_defaults() -> Self {
        Self::new()
    }

    fn has_lowercase(password: &[u8]) -> bool {
        password.iter().any(|byte| byte.is_ascii_lowercase())
    }

    fn has_uppercase(password: &[u8]) -> bool {
        password.iter().any(|byte| byte.is_ascii_uppercase())
    }

    fn has_digit(password: &[u8]) -> bool {
        password.iter().any(|byte| byte.is_ascii_digit())
    }

    fn has_symbol(password: &[u8]) -> bool {
        password
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && !byte.is_ascii_whitespace())
    }

    fn has_whitespace(password: &[u8]) -> bool {
        password.iter().any(|byte| byte.is_ascii_whitespace())
    }

    fn validate_length(length: usize) -> Result<(), AppError> {
        if length < MIN_PASSWORD_LENGTH {
            return Err(AppError::Validation(format!(
                "password length must be at least {MIN_PASSWORD_LENGTH} characters"
            )));
        }

        if length > MAX_PASSWORD_LENGTH {
            return Err(AppError::Validation(format!(
                "password length must not exceed {MAX_PASSWORD_LENGTH} characters"
            )));
        }

        Ok(())
    }
}

impl Default for PasswordServiceImpl {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl PasswordService for PasswordServiceImpl {
    fn validate_password_policy(&self, password: &SecretBox<Vec<u8>>) -> Result<(), AppError> {
        let password_bytes = password.expose_secret().as_slice();

        Self::validate_length(password_bytes.len())?;

        if Self::has_whitespace(password_bytes) {
            return Err(AppError::Validation(
                "password must not contain whitespace".to_string(),
            ));
        }

        if !Self::has_lowercase(password_bytes) {
            return Err(AppError::Validation(
                "password must contain at least one lowercase character".to_string(),
            ));
        }

        if !Self::has_uppercase(password_bytes) {
            return Err(AppError::Validation(
                "password must contain at least one uppercase character".to_string(),
            ));
        }

        if !Self::has_digit(password_bytes) {
            return Err(AppError::Validation(
                "password must contain at least one digit".to_string(),
            ));
        }

        if !Self::has_symbol(password_bytes) {
            return Err(AppError::Validation(
                "password must contain at least one symbol".to_string(),
            ));
        }

        Ok(())
    }

    fn score_password_strength(&self, password: &SecretBox<Vec<u8>>) -> Result<u8, AppError> {
        let password_bytes = password.expose_secret().as_slice();

        if password_bytes.is_empty() {
            return Err(AppError::Validation(
                "password must not be empty".to_string(),
            ));
        }

        let mut score: u16 = 0;
        let length = password_bytes.len();

        if length >= 8 {
            score += 10;
        }
        if length >= 12 {
            score += 15;
        }
        if length >= MIN_PASSWORD_LENGTH {
            score += 20;
        }
        if length >= 24 {
            score += 15;
        }

        if Self::has_lowercase(password_bytes) {
            score += 10;
        }
        if Self::has_uppercase(password_bytes) {
            score += 10;
        }
        if Self::has_digit(password_bytes) {
            score += 10;
        }
        if Self::has_symbol(password_bytes) {
            score += 10;
        }

        let mut seen = [false; 256];
        let mut unique_count: u16 = 0;
        for byte in password_bytes {
            let index = usize::from(*byte);
            if !seen[index] {
                seen[index] = true;
                unique_count += 1;
            }
        }

        let password_len_u16 = u16::try_from(length).map_err(|_| {
            AppError::Validation("password length exceeds supported size".to_string())
        })?;
        if unique_count.saturating_mul(2) >= password_len_u16 {
            score += 10;
        }

        if Self::has_whitespace(password_bytes) && score >= 10 {
            score -= 10;
        }

        let capped = score.min(u16::from(PASSWORD_STRENGTH_MAX));
        let final_score = u8::try_from(capped)
            .map_err(|_| AppError::Validation("password score conversion failed".to_string()))?;

        Ok(final_score)
    }

    fn generate_password(&self, length: usize) -> Result<SecretBox<Vec<u8>>, AppError> {
        let requested_length = if length == 0 {
            DEFAULT_GENERATED_PASSWORD_LENGTH
        } else {
            length
        };

        Self::validate_length(requested_length)?;

        let length = u16::try_from(requested_length).map_err(|_| {
            AppError::Validation("password length exceeds supported size".to_string())
        })?;
        let recipe = PasswordRecipe {
            length,
            ..PasswordRecipe::default()
        };
        let secret_password = GeneratorRecipe::Password(recipe).generate()?;
        PasswordService::validate_password_policy(self, &secret_password)?;
        Ok(secret_password)
    }
}

#[cfg(test)]
mod tests {
    use secrecy::{ExposeSecret, SecretBox};

    use crate::errors::AppError;

    use super::{PasswordService, PasswordServiceImpl};

    #[test]
    fn validate_policy_accepts_strong_password() {
        let service = PasswordServiceImpl::new();
        let password = SecretBox::new(Box::new(b"Str0ng!Password42".to_vec()));

        let result = service.validate_password_policy(&password);
        assert!(result.is_ok(), "strong password should satisfy policy");
    }

    #[test]
    fn validate_policy_rejects_missing_symbol() {
        let service = PasswordServiceImpl::new();
        let password = SecretBox::new(Box::new(b"StrongPassword42".to_vec()));

        let result = service.validate_password_policy(&password);
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn scoring_rewards_longer_and_more_diverse_passwords() {
        let service = PasswordServiceImpl::new();
        let weak_password = SecretBox::new(Box::new(b"password".to_vec()));
        let strong_password = SecretBox::new(Box::new(b"C0mpl3x!Passphrase#2026".to_vec()));

        let weak_score_result = service.score_password_strength(&weak_password);
        let strong_score_result = service.score_password_strength(&strong_password);

        assert!(
            weak_score_result.is_ok(),
            "weak password score should still compute"
        );
        assert!(
            strong_score_result.is_ok(),
            "strong password score should compute"
        );

        let weak_score = match weak_score_result {
            Ok(value) => value,
            Err(_) => return,
        };
        let strong_score = match strong_score_result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert!(strong_score > weak_score);
    }

    #[test]
    fn generate_password_returns_policy_compliant_secret() {
        let service = PasswordServiceImpl::new();

        let generated_result = service.generate_password(24);
        assert!(
            generated_result.is_ok(),
            "password generation should succeed"
        );
        let generated = match generated_result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(generated.expose_secret().len(), 24);

        let validation_result = service.validate_password_policy(&generated);
        assert!(
            validation_result.is_ok(),
            "generated password should satisfy policy"
        );
    }

    #[test]
    fn generate_password_uses_default_length_when_zero_requested() {
        let service = PasswordServiceImpl::new();

        let generated_result = service.generate_password(0);
        assert!(
            generated_result.is_ok(),
            "default password generation should succeed"
        );
        let generated = match generated_result {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(generated.expose_secret().len(), 24);
    }
}

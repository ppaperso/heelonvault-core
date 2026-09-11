//! The account key: a random key that wraps every vault key of an account. It is stored only
//! encrypted, under the password (see `auth_service`) and under the recovery phrase (here).

use secrecy::{ExposeSecret, SecretBox, SecretString};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::errors::AppError;
use crate::repositories::user_repository::UserKeyMaterialRepository;
use crate::services::crypto_service::{CryptoService, EncryptedPayload, KDF_SALT_LEN, NONCE_LEN};

pub const ACCOUNT_KEY_LEN: usize = 32;
const RECOVERY_KEY_ENVELOPE_VERSION: u8 = 1;
const RECOVERY_KEY_HEADER_LEN: usize = 1 + KDF_SALT_LEN + NONCE_LEN;
/// Keeps this derivation distinct from the `.hvb` key and the phrase verifier.
const RECOVERY_KEK_DOMAIN: &str = "heelonvault/recovery-key/v1\n";

pub fn generate_account_key() -> Result<SecretBox<Vec<u8>>, AppError> {
    let mut key = Zeroizing::new(vec![0_u8; ACCOUNT_KEY_LEN]);
    getrandom::fill(key.as_mut_slice())
        .map_err(|err| AppError::Crypto(format!("account key generation failed: {err}")))?;
    Ok(SecretBox::new(Box::new(key.to_vec())))
}

async fn derive_recovery_kek<C>(
    crypto: &C,
    phrase: &SecretString,
    salt: &SecretBox<Vec<u8>>,
) -> Result<SecretBox<Vec<u8>>, AppError>
where
    C: CryptoService + Send + Sync,
{
    // Lower case and single spaces: the phrase must open whatever way it is retyped.
    let words = phrase
        .expose_secret()
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ");
    let mut input = Zeroizing::new(String::with_capacity(
        RECOVERY_KEK_DOMAIN.len() + words.len(),
    ));
    input.push_str(RECOVERY_KEK_DOMAIN);
    input.push_str(words.as_str());
    drop(Zeroizing::new(words));
    crypto
        .derive_key(&SecretString::new(input.as_str().into()), salt)
        .await
}

/// `version || salt || nonce || ciphertext`, openable with the recovery phrase alone.
pub async fn seal_with_recovery_phrase<C>(
    crypto: &C,
    phrase: &SecretString,
    account_key: &SecretBox<Vec<u8>>,
) -> Result<SecretBox<Vec<u8>>, AppError>
where
    C: CryptoService + Send + Sync,
{
    let salt = crypto.generate_kdf_salt().await?;
    if salt.expose_secret().len() != KDF_SALT_LEN {
        return Err(AppError::Crypto(
            "unexpected salt length for the recovery key".to_string(),
        ));
    }
    let kek = derive_recovery_kek(crypto, phrase, &salt).await?;
    let payload = crypto.encrypt(account_key, &kek).await?;

    let ciphertext = payload.ciphertext.expose_secret();
    let mut envelope = Vec::with_capacity(RECOVERY_KEY_HEADER_LEN + ciphertext.len());
    envelope.push(RECOVERY_KEY_ENVELOPE_VERSION);
    envelope.extend_from_slice(salt.expose_secret());
    envelope.extend_from_slice(&payload.nonce);
    envelope.extend_from_slice(ciphertext);
    Ok(SecretBox::new(Box::new(envelope)))
}

/// Fails when `phrase` is not the one the envelope was sealed with.
pub async fn open_with_recovery_phrase<C>(
    crypto: &C,
    phrase: &SecretString,
    envelope: &SecretBox<Vec<u8>>,
) -> Result<SecretBox<Vec<u8>>, AppError>
where
    C: CryptoService + Send + Sync,
{
    let bytes = envelope.expose_secret();
    if bytes.len() <= RECOVERY_KEY_HEADER_LEN || bytes[0] != RECOVERY_KEY_ENVELOPE_VERSION {
        return Err(AppError::Validation(
            "recovery key envelope is malformed".to_string(),
        ));
    }

    let salt = SecretBox::new(Box::new(bytes[1..=KDF_SALT_LEN].to_vec()));
    let mut nonce = [0_u8; NONCE_LEN];
    nonce.copy_from_slice(&bytes[1 + KDF_SALT_LEN..RECOVERY_KEY_HEADER_LEN]);
    let payload = EncryptedPayload {
        nonce,
        ciphertext: SecretBox::new(Box::new(bytes[RECOVERY_KEY_HEADER_LEN..].to_vec())),
    };

    let kek = derive_recovery_kek(crypto, phrase, &salt).await?;
    crypto.decrypt(&payload, &kek).await
}

/// Reseals the account key with `phrase` unless the stored envelope already opens to it.
/// Returns whether a new envelope was written.
pub async fn ensure_recovery_key<U, C>(
    users: &U,
    crypto: &C,
    user_id: Uuid,
    account_key: &SecretBox<Vec<u8>>,
    phrase: &SecretString,
) -> Result<bool, AppError>
where
    U: UserKeyMaterialRepository + Send + Sync,
    C: CryptoService + Send + Sync,
{
    if let Some(existing) = users.get_recovery_key_envelope(user_id).await?
        && let Ok(opened) = open_with_recovery_phrase(crypto, phrase, &existing).await
        && bool::from(
            opened
                .expose_secret()
                .as_slice()
                .ct_eq(account_key.expose_secret().as_slice()),
        )
    {
        return Ok(false);
    }

    let envelope = seal_with_recovery_phrase(crypto, phrase, account_key).await?;
    users.set_recovery_key_envelope(user_id, envelope).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    // `#[tokio::test]` expands to a runtime builder that calls `expect`.
    #![allow(clippy::disallowed_methods)]
    use secrecy::{ExposeSecret, SecretString};

    use crate::errors::AppError;
    use crate::services::crypto_service::{CryptoServiceImpl, KdfConfig};

    use super::{generate_account_key, open_with_recovery_phrase, seal_with_recovery_phrase};

    const PHRASE: &str =
        "abandon ability able about above absent absorb abstract absurd abuse access accident";

    fn fast_crypto() -> CryptoServiceImpl {
        CryptoServiceImpl::new(KdfConfig {
            memory_cost_kib: 1024,
            time_cost: 1,
            parallelism: 1,
            output_len: 32,
        })
    }

    #[tokio::test]
    async fn sealed_key_opens_with_the_phrase_however_it_is_retyped() -> Result<(), AppError> {
        let crypto = fast_crypto();
        let account_key = generate_account_key()?;
        let sealed =
            seal_with_recovery_phrase(&crypto, &SecretString::new(PHRASE.into()), &account_key)
                .await?;

        let retyped = format!("  {}  ", PHRASE.to_uppercase().replace(' ', "   "));
        let opened =
            open_with_recovery_phrase(&crypto, &SecretString::new(retyped.into()), &sealed).await?;

        assert_eq!(opened.expose_secret(), account_key.expose_secret());
        Ok(())
    }

    #[tokio::test]
    async fn sealed_key_does_not_open_with_another_phrase() -> Result<(), AppError> {
        let crypto = fast_crypto();
        let account_key = generate_account_key()?;
        let sealed =
            seal_with_recovery_phrase(&crypto, &SecretString::new(PHRASE.into()), &account_key)
                .await?;

        let other = SecretString::new("zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong".into());
        assert!(
            open_with_recovery_phrase(&crypto, &other, &sealed)
                .await
                .is_err()
        );
        Ok(())
    }
}

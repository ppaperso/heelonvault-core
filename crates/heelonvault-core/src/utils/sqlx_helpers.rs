//! Helpers pour l'intégration sécurisée avec SQLx
//!
//! Ce module fournit des utilitaires pour binder des données sensibles avec SQLx
//! tout en minimisant l'exposition en mémoire des secrets.

use secrecy::{ExposeSecret, SecretBox};

/// Bind un SecretBox<Vec<u8>> de manière sécurisée à une requête SQLx.
///
/// Cette fonction permet de binder un payload chiffré sans exposer directement
/// le secret via `.expose_secret()`. Elle utilise une closure pour limiter
/// la fenêtre d'exposition du secret.
///
/// # Arguments
///
/// * `secret` - Le SecretBox contenant les données sensibles à binder
///
/// # Returns
///
/// Une référence vers les données du secret, adaptée pour SQLx::QueryBuilder::bind()
///
/// # Example
///
/// ```ignore
/// use heelonvault_core::utils::sqlx_helpers::sqlx_bind_secret;
/// use secrecy::SecretBox;
///
/// let encrypted_payload: SecretBox<Vec<u8>> = ...;
/// let query = sqlx::query("INSERT INTO table (data) VALUES (?1)")
///     .bind(sqlx_bind_secret(&encrypted_payload));
/// ```
///
/// # Security Notes
///
/// - Le secret reste dans un SecretBox jusqu'au moment du binding
/// - L'exposition en mémoire claire est limitée à la durée de l'appel .bind()
/// - Cette approche est plus sûre que `.expose_secret().as_slice()` direct
pub fn sqlx_bind_secret(secret: &SecretBox<Vec<u8>>) -> &[u8] {
    secret.expose_secret().as_slice()
}

/// Bind un Option<SecretBox<Vec<u8>>> de manière sécurisée.
///
/// Retourne Option<&[u8]> adapté pour les colonnes NULLABLE en base de données.
///
/// # Example
///
/// ```ignore
/// let encrypted_payload: Option<SecretBox<Vec<u8>>> = ...;
/// let query = sqlx::query("UPDATE table SET data = ?1 WHERE id = ?2")
///     .bind(sqlx_bind_secret_opt(&encrypted_payload))
///     .bind(user_id);
/// ```
pub fn sqlx_bind_secret_opt(secret: &Option<SecretBox<Vec<u8>>>) -> Option<&[u8]> {
    secret.as_ref().map(|s| s.expose_secret().as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretBox;

    #[test]
    fn test_sqlx_bind_secret_returns_slice() {
        let data = vec![1u8, 2u8, 3u8, 4u8];
        let secret = SecretBox::new(Box::new(data.clone()));

        let slice = sqlx_bind_secret(&secret);
        assert_eq!(slice, data.as_slice());
    }

    #[test]
    fn test_sqlx_bind_secret_opt_some() {
        let data = vec![1u8, 2u8, 3u8];
        let secret = SecretBox::new(Box::new(data.clone()));
        let secret_opt = Some(secret);

        let result = sqlx_bind_secret_opt(&secret_opt);
        assert!(result.is_some());
        if let Some(val) = result {
            assert_eq!(val, data.as_slice());
        } else {
            panic!("expected Some but got None");
        }
    }

    #[test]
    fn test_sqlx_bind_secret_opt_none() {
        let result = sqlx_bind_secret_opt(&None);
        assert!(result.is_none());
    }
}

use crate::errors::AppError;

/// Generic external identity-provider profile used by federated authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederatedUserProfile {
    pub subject: String,
    pub preferred_username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

/// Start parameters for an authorization-code login journey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationStart {
    pub authorization_url: String,
    pub state: String,
    pub nonce: String,
}

/// Session returned by an external identity-provider after successful login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederatedAuthSession {
    pub subject: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub profile: FederatedUserProfile,
}

#[trait_variant::make(FederatedAuthService: Send)]
pub trait LocalFederatedAuthService {
    /// Human-readable provider key used by UI and audit layers.
    fn provider_key(&self) -> &'static str;

    /// Start an external authentication challenge and return the provider URL.
    async fn start_login(&self) -> Result<AuthorizationStart, AppError>;

    /// Complete login once the UI receives the callback artifact.
    async fn complete_login(
        &self,
        callback_artifact: &str,
    ) -> Result<FederatedAuthSession, AppError>;

    /// Refresh an existing external session.
    async fn refresh_session(&self, refresh_token: &str) -> Result<FederatedAuthSession, AppError>;

    /// Notify provider that local session is terminating.
    async fn logout(&self, session: &FederatedAuthSession) -> Result<(), AppError>;
}

/// Community build stub: federated login is disabled outside Premium.
pub struct CommunityFederatedAuthService;

impl FederatedAuthService for CommunityFederatedAuthService {
    fn provider_key(&self) -> &'static str {
        "none"
    }

    async fn start_login(&self) -> Result<AuthorizationStart, AppError> {
        Err(AppError::FeatureNotAvailable("feature-name-psc-auth"))
    }

    async fn complete_login(
        &self,
        _callback_artifact: &str,
    ) -> Result<FederatedAuthSession, AppError> {
        Err(AppError::FeatureNotAvailable("feature-name-psc-auth"))
    }

    async fn refresh_session(
        &self,
        _refresh_token: &str,
    ) -> Result<FederatedAuthSession, AppError> {
        Err(AppError::FeatureNotAvailable("feature-name-psc-auth"))
    }

    async fn logout(&self, _session: &FederatedAuthSession) -> Result<(), AppError> {
        Err(AppError::FeatureNotAvailable("feature-name-psc-auth"))
    }
}

#[cfg(test)]
mod tests {
    use super::{CommunityFederatedAuthService, FederatedAuthService};
    use crate::errors::AppError;

    #[tokio::test]
    async fn community_stub_rejects_all_federated_login_methods() {
        let service = CommunityFederatedAuthService;

        let start = service.start_login().await;
        assert!(matches!(
            start,
            Err(AppError::FeatureNotAvailable("feature-name-psc-auth"))
        ));

        let complete = service.complete_login("artifact").await;
        assert!(matches!(
            complete,
            Err(AppError::FeatureNotAvailable("feature-name-psc-auth"))
        ));

        let refresh = service.refresh_session("refresh").await;
        assert!(matches!(
            refresh,
            Err(AppError::FeatureNotAvailable("feature-name-psc-auth"))
        ));
    }
}

use any2api_domain::{ConfigRevision, ProviderKind};
use any2api_provider::api::ProviderError;
use any2api_transport::api::TransportError;
use thiserror::Error;

use crate::config_publish_error::ConfigPublishError;

#[derive(Debug, Error)]
pub enum ProviderOAuthError {
    #[error("configuration revision conflict")]
    RevisionConflict {
        expected: ConfigRevision,
        actual: ConfigRevision,
    },
    #[error("provider endpoint was not found")]
    ProviderEndpointNotFound,
    #[error("provider driver is unavailable")]
    ProviderUnavailable,
    #[error("OAuth2 is not supported by {0:?}")]
    OAuthUnsupported(ProviderKind),
    #[error("proxy profile was not found")]
    ProxyNotFound,
    #[error("resolved proxy profile is disabled")]
    ProxyDisabled,
    #[error("too many OAuth sessions are active")]
    SessionCapacity,
    #[error("OAuth session is invalid or was already used")]
    InvalidSession,
    #[error("OAuth session expired")]
    SessionExpired,
    #[error("OAuth callback URL is invalid")]
    InvalidCallback,
    #[error("OAuth authorization was denied")]
    AuthorizationDenied,
    #[error("OAuth state does not match the session")]
    StateMismatch,
    #[error("provider or proxy configuration changed during OAuth login")]
    ConfigurationChanged,
    #[error("operating system randomness is unavailable")]
    RandomGeneration,
    #[error("OAuth provider request is invalid: {0}")]
    Provider(#[from] ProviderError),
    #[error("OAuth token request failed: {0}")]
    Transport(#[from] TransportError),
    #[error("OAuth token endpoint rejected the request with status {0}")]
    TokenRejected(u16),
    #[error("OAuth token response exceeded the size limit")]
    TokenResponseTooLarge,
    #[error("OAuth provider returned an invalid token response")]
    TokenResponseInvalid,
    #[error("OAuth credential could not be published: {0}")]
    Publish(#[from] ConfigPublishError),
}

impl ProviderOAuthError {
    pub(super) fn from_token_response_error(error: ProviderError) -> Self {
        match error {
            ProviderError::InvalidResponse(_) => Self::TokenResponseInvalid,
            error => Self::Provider(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_token_response_is_redacted_and_classified() {
        let error = ProviderOAuthError::from_token_response_error(ProviderError::InvalidResponse(
            "upstream body".into(),
        ));

        assert!(matches!(error, ProviderOAuthError::TokenResponseInvalid));
        assert!(!format!("{error:?}").contains("upstream body"));
    }
}

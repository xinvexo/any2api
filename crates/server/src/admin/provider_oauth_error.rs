use any2api_runtime::api::ProviderOAuthError;
use axum::http::StatusCode;

use super::error::AdminApiError;

impl From<ProviderOAuthError> for AdminApiError {
    fn from(error: ProviderOAuthError) -> Self {
        match error {
            ProviderOAuthError::RevisionConflict { .. } => Self::new(
                StatusCode::CONFLICT,
                "revision_conflict",
                "configuration changed; refresh and try again",
            ),
            ProviderOAuthError::ProviderEndpointNotFound => Self::provider_endpoint_not_found(),
            ProviderOAuthError::OAuthUnsupported(_) => Self::new(
                StatusCode::BAD_REQUEST,
                "provider_oauth_unsupported",
                "this provider does not support OAuth login",
            ),
            ProviderOAuthError::ProxyNotFound | ProviderOAuthError::ProxyDisabled => Self::new(
                StatusCode::CONFLICT,
                "provider_oauth_proxy_unavailable",
                "the selected proxy is unavailable",
            ),
            ProviderOAuthError::SessionCapacity => Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "provider_oauth_session_capacity",
                "too many OAuth login sessions are active; try again shortly",
            ),
            ProviderOAuthError::InvalidSession => Self::new(
                StatusCode::BAD_REQUEST,
                "provider_oauth_session_invalid",
                "the OAuth login session is invalid or was already used",
            ),
            ProviderOAuthError::SessionExpired => Self::new(
                StatusCode::GONE,
                "provider_oauth_session_expired",
                "the OAuth login session expired; start again",
            ),
            ProviderOAuthError::InvalidCallback => Self::new(
                StatusCode::BAD_REQUEST,
                "provider_oauth_callback_invalid",
                "paste the complete OAuth callback URL",
            ),
            ProviderOAuthError::AuthorizationDenied => Self::new(
                StatusCode::BAD_REQUEST,
                "provider_oauth_authorization_denied",
                "OAuth authorization was denied",
            ),
            ProviderOAuthError::StateMismatch => Self::new(
                StatusCode::FORBIDDEN,
                "provider_oauth_state_mismatch",
                "the OAuth callback does not belong to this login session",
            ),
            ProviderOAuthError::ConfigurationChanged => Self::new(
                StatusCode::CONFLICT,
                "provider_oauth_configuration_changed",
                "the provider or proxy changed during login; start again",
            ),
            ProviderOAuthError::Publish(error) => error.into(),
            ProviderOAuthError::TokenRejected(_)
            | ProviderOAuthError::Transport(_)
            | ProviderOAuthError::TokenResponseTooLarge
            | ProviderOAuthError::TokenResponseInvalid => Self::new(
                StatusCode::BAD_GATEWAY,
                "provider_oauth_upstream_failed",
                "the OAuth provider could not complete the token exchange",
            ),
            ProviderOAuthError::ProviderUnavailable
            | ProviderOAuthError::RandomGeneration
            | ProviderOAuthError::Provider(_) => {
                tracing::error!(?error, "provider OAuth login failed internally");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "provider OAuth login could not be completed",
                )
            }
        }
    }
}

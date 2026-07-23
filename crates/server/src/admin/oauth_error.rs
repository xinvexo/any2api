use any2api_runtime::api::OAuthError;
use axum::http::StatusCode;

use super::error::AdminApiError;

pub(super) fn map(error: OAuthError) -> AdminApiError {
    match error {
        OAuthError::ProviderUnavailable => AdminApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "oauth_unavailable",
            "OAuth2 login is unavailable",
        ),
        OAuthError::UnsupportedProvider(_) => AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_provider_unsupported",
            "the selected provider does not support OAuth2 login",
        ),
        OAuthError::SessionCapacity => AdminApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "oauth_session_capacity",
            "too many OAuth2 login sessions are active",
        ),
        OAuthError::InvalidSession => AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_session_invalid",
            "the OAuth2 login session is invalid or was already used",
        ),
        OAuthError::SessionExpired => AdminApiError::new(
            StatusCode::GONE,
            "oauth_session_expired",
            "the OAuth2 login session expired",
        ),
        OAuthError::InvalidCallback => AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_callback_invalid",
            "the OAuth2 callback URL is invalid",
        ),
        OAuthError::AuthorizationDenied => AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_authorization_denied",
            "OAuth2 authorization was denied",
        ),
        OAuthError::StateMismatch => AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_state_mismatch",
            "the OAuth2 callback state is invalid",
        ),
        OAuthError::TokenReadTimeout => AdminApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "oauth_token_timeout",
            "the OAuth2 token endpoint timed out",
        ),
        OAuthError::TokenResponseTooLarge
        | OAuthError::TokenResponseInvalid
        | OAuthError::TokenRejected(_)
        | OAuthError::Transport(_)
        | OAuthError::Provider(_) => {
            tracing::warn!(error = ?error, "OAuth2 token exchange failed");
            AdminApiError::new(
                StatusCode::BAD_GATEWAY,
                "oauth_token_exchange_failed",
                "the OAuth2 token exchange failed",
            )
        }
        OAuthError::RandomGeneration | OAuthError::FileSerialization => {
            tracing::error!(error = ?error, "OAuth2 login could not be completed");
            AdminApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "oauth_internal_error",
                "OAuth2 login could not be completed",
            )
        }
    }
}

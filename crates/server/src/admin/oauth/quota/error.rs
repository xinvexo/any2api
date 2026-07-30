use any2api_runtime::api::OAuthQuotaError;
use axum::http::StatusCode;

use crate::admin::error::AdminApiError;

pub(super) fn map(error: OAuthQuotaError) -> AdminApiError {
    match error {
        OAuthQuotaError::AccountNotFound => AdminApiError::new(
            StatusCode::NOT_FOUND,
            "oauth_account_not_found",
            "OAuth account was not found",
        ),
        OAuthQuotaError::UnsupportedProvider => AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_quota_unsupported",
            "quota management is not supported for this OAuth provider",
        ),
        OAuthQuotaError::CredentialRateLimited => AdminApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "oauth_account_rate_limited",
            "OAuth account has exhausted its local RPM limit",
        ),
        OAuthQuotaError::NoResetCredits => AdminApiError::new(
            StatusCode::CONFLICT,
            "oauth_quota_reset_unavailable",
            "OAuth account has no available quota reset credits",
        ),
        OAuthQuotaError::ReadTimeout => AdminApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "oauth_quota_timeout",
            "the OAuth quota request timed out",
        ),
        OAuthQuotaError::AuthenticationRefreshFailed => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_account_authentication_unverified",
            "the upstream rejected this OAuth account, but token refresh could not be completed",
        ),
        OAuthQuotaError::AuthenticationFailed => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_account_authentication_failed",
            "the upstream definitively rejected this OAuth account authentication",
        ),
        OAuthQuotaError::AccountRestricted => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_account_restricted",
            "the upstream provider restricted this OAuth account",
        ),
        OAuthQuotaError::UpstreamRejected(_)
        | OAuthQuotaError::ResponseTooLarge
        | OAuthQuotaError::Provider(_)
        | OAuthQuotaError::Transport(_) => {
            tracing::warn!(error = ?error, "OAuth quota upstream request failed");
            AdminApiError::new(
                StatusCode::BAD_GATEWAY,
                "oauth_quota_upstream_failed",
                "the OAuth quota upstream request failed",
            )
        }
        OAuthQuotaError::ProviderUnavailable
        | OAuthQuotaError::RuntimeUnavailable
        | OAuthQuotaError::TokenMaterialUnavailable
        | OAuthQuotaError::ProxyUnavailable => AdminApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "oauth_quota_unavailable",
            "OAuth quota management is unavailable",
        ),
        OAuthQuotaError::InvalidEndpointUri => {
            tracing::error!(error = ?error, "OAuth quota request could not be constructed");
            AdminApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "oauth_quota_internal_error",
                "OAuth quota management could not be completed",
            )
        }
    }
}

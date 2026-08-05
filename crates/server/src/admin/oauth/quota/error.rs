use any2api_runtime::api::OAuthQuotaError;
use axum::http::StatusCode;

use crate::admin::error::AdminApiError;

use super::super::refresh_diagnostic::error_diagnostic;

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
        OAuthQuotaError::RefreshTokenMissing(failure) => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_refresh_token_missing",
            "the access token was rejected and this account has no refresh token",
        )
        .with_diagnostic(error_diagnostic(failure)),
        OAuthQuotaError::RefreshPermanentlyRejected(failure) => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_refresh_permanently_rejected",
            "the OAuth provider permanently rejected this account's refresh token",
        )
        .with_diagnostic(error_diagnostic(failure)),
        OAuthQuotaError::TokenRefreshFailed(failure) => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_token_refresh_failed",
            "the OAuth token refresh failed before authentication could be verified",
        )
        .with_diagnostic(error_diagnostic(failure)),
        OAuthQuotaError::RefreshedAccessTokenRejected(failure) => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_refreshed_access_token_rejected",
            "the OAuth provider rejected the newly refreshed access token",
        )
        .with_diagnostic(error_diagnostic(failure)),
        OAuthQuotaError::AccountRestricted => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_account_restricted",
            "the upstream provider restricted this OAuth account",
        ),
        OAuthQuotaError::ProviderEgressRestricted => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "oauth_provider_egress_restricted",
            "the OAuth provider rejected the current network egress",
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
        OAuthQuotaError::InvalidEndpointUri
        | OAuthQuotaError::Persistence(_)
        | OAuthQuotaError::InvalidPersistedSnapshot => {
            tracing::error!(error = ?error, "OAuth quota request could not be constructed");
            AdminApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "oauth_quota_internal_error",
                "OAuth quota management could not be completed",
            )
        }
    }
}

use any2api_runtime::api::ProviderCredentialTestError;
use axum::http::StatusCode;

pub(super) use crate::admin::error::AdminApiError;

impl From<ProviderCredentialTestError> for AdminApiError {
    fn from(error: ProviderCredentialTestError) -> Self {
        match error {
            ProviderCredentialTestError::CredentialNotFound => {
                Self::provider_credential_not_found()
            }
            ProviderCredentialTestError::CredentialDisabled => Self::new(
                StatusCode::CONFLICT,
                "provider_credential_disabled",
                "a disabled provider credential cannot be tested",
            ),
            ProviderCredentialTestError::ProviderEndpointNotFound => {
                Self::provider_endpoint_not_found()
            }
            ProviderCredentialTestError::ProviderEndpointDisabled => Self::new(
                StatusCode::CONFLICT,
                "provider_endpoint_disabled",
                "a provider credential with a disabled endpoint cannot be tested",
            ),
            ProviderCredentialTestError::ProxyNotFound
            | ProviderCredentialTestError::ProxyDisabled => Self::new(
                StatusCode::CONFLICT,
                "provider_credential_proxy_unavailable",
                "the provider credential's resolved proxy is unavailable",
            ),
            ProviderCredentialTestError::CredentialRateLimited => Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "provider_credential_rate_limited",
                "the provider credential has exhausted its local RPM limit",
            ),
            ProviderCredentialTestError::CredentialRuntimeUnavailable
            | ProviderCredentialTestError::ProviderUnavailable
            | ProviderCredentialTestError::InvalidEndpointUri
            | ProviderCredentialTestError::Provider(_) => {
                tracing::error!(?error, "provider credential test could not be prepared");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "provider credential test could not be started",
                )
            }
        }
    }
}

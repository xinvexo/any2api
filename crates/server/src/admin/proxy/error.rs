use any2api_runtime::api::ProxyTestError;
use axum::http::StatusCode;

pub(super) use crate::admin::error::AdminApiError;

impl From<ProxyTestError> for AdminApiError {
    fn from(error: ProxyTestError) -> Self {
        match error {
            ProxyTestError::ProxyNotFound => Self::new(
                StatusCode::NOT_FOUND,
                "proxy_not_found",
                "proxy profile was not found",
            ),
            ProxyTestError::ProxyDisabled => Self::new(
                StatusCode::CONFLICT,
                "proxy_disabled",
                "a disabled proxy cannot be tested",
            ),
            ProxyTestError::ProviderEndpointNotFound => Self::provider_endpoint_not_found(),
            ProxyTestError::InvalidEndpointUri => {
                tracing::error!("published provider endpoint URI is invalid");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "proxy test could not be started",
                )
            }
        }
    }
}

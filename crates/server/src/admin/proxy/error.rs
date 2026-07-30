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
        }
    }
}

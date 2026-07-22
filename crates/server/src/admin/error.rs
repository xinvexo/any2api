use any2api_runtime::api::{ProviderCredentialTestError, ProxyTestError};
use axum::{
    Json,
    http::{
        HeaderValue, StatusCode,
        header::{CACHE_CONTROL, RETRY_AFTER},
    },
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct AdminApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    retry_after: Option<u64>,
}

impl AdminApiError {
    pub(crate) fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub(crate) fn invalid_provider_endpoint(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_provider_endpoint",
            message,
        )
    }

    pub(crate) fn invalid_provider_credential(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_provider_credential",
            message,
        )
    }

    pub(crate) fn invalid_model_route(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_model_route", message)
    }

    pub(crate) fn invalid_gateway_api_key(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_gateway_api_key", message)
    }

    pub(crate) fn invalid_setting(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_setting", message)
    }

    pub(crate) fn provider_endpoint_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "provider_endpoint_not_found",
            "provider endpoint was not found",
        )
    }

    pub(crate) fn provider_credential_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "provider_credential_not_found",
            "provider credential was not found",
        )
    }

    pub(crate) fn model_route_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "model_route_not_found",
            "model route was not found",
        )
    }

    pub(crate) fn setting_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "setting_not_found",
            "setting was not found",
        )
    }

    pub(crate) fn request_log_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "request_log_not_found",
            "request log was not found",
        )
    }

    pub(crate) fn request_log_unavailable() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "request_log_unavailable",
            "request logs could not be read",
        )
    }

    pub(crate) fn proxy_test_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "proxy_test_unavailable",
            "proxy testing is unavailable",
        )
    }

    pub(crate) fn provider_credential_test_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "provider_credential_test_unavailable",
            "provider credential testing is unavailable",
        )
    }

    pub(crate) fn loopback_only() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "admin_loopback_only",
            "administrator authentication is not configured; use a loopback connection",
        )
    }

    pub(crate) fn remote_disabled() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "admin_remote_disabled",
            "remote administrator access is disabled",
        )
    }

    pub(crate) fn invalid_forwarded_headers() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "admin_invalid_forwarded_headers",
            "trusted proxy headers are invalid",
        )
    }

    pub(crate) fn auth_unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "admin_auth_unavailable",
            "administrator authentication is unavailable",
        )
    }

    pub(crate) fn shutting_down() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "server_shutting_down",
            "service is shutting down",
        )
    }

    pub(crate) fn setup_loopback_only() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "admin_setup_loopback_only",
            "administrator setup requires a loopback connection",
        )
    }

    pub(crate) fn already_initialized() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "admin_already_initialized",
            "administrator password is already initialized",
        )
    }

    pub(crate) fn setup_required() -> Self {
        Self::new(
            StatusCode::PRECONDITION_REQUIRED,
            "admin_setup_required",
            "administrator password must be initialized from loopback first",
        )
    }

    pub(crate) fn invalid_admin_password() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "admin_invalid_password",
            "administrator password must contain between 12 and 1024 bytes",
        )
    }

    pub(crate) fn invalid_setup_token() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "admin_invalid_setup_token",
            "administrator setup token is invalid",
        )
    }

    pub(crate) fn invalid_credentials() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "admin_invalid_credentials",
            "administrator credentials are invalid",
        )
    }

    pub(crate) fn current_password_invalid() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "admin_current_password_invalid",
            "the current administrator password is invalid",
        )
    }

    pub(crate) fn password_changed() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "admin_password_changed",
            "administrator credentials changed; sign in again and retry",
        )
    }

    pub(crate) fn session_required() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "admin_session_required",
            "administrator session is required",
        )
    }

    pub(crate) fn csrf_invalid() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "admin_csrf_invalid",
            "administrator CSRF token is missing or invalid",
        )
    }

    pub(crate) fn login_rate_limited(retry_after: u64) -> Self {
        let mut error = Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "admin_login_rate_limited",
            "too many administrator login failures; try again later",
        );
        error.retry_after = Some(retry_after);
        error
    }

    pub(crate) fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "administrator authentication failed",
        )
    }

    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }
}

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
            ProviderCredentialTestError::CredentialAtCapacity => Self::new(
                StatusCode::CONFLICT,
                "provider_credential_at_capacity",
                "the provider credential is at capacity",
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

impl IntoResponse for AdminApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
            },
        };

        let mut response = (self.status, Json(body)).into_response();
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if let Some(retry_after) = self.retry_after
            && let Ok(value) = HeaderValue::from_str(&retry_after.to_string())
        {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        response
    }
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

pub(crate) async fn not_found() -> AdminApiError {
    AdminApiError::new(
        StatusCode::NOT_FOUND,
        "admin_api_not_found",
        "administrator API route was not found",
    )
}

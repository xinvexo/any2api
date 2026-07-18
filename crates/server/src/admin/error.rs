use any2api_runtime::api::ConfigPublishError;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct AdminApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
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

    pub(crate) fn loopback_only() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "admin_loopback_only",
            "administrator authentication is not configured; use a loopback connection",
        )
    }

    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl From<ConfigPublishError> for AdminApiError {
    fn from(error: ConfigPublishError) -> Self {
        match error {
            ConfigPublishError::RevisionConflict { .. } => Self::new(
                StatusCode::CONFLICT,
                "revision_conflict",
                "configuration changed; refresh and try again",
            ),
            ConfigPublishError::ProxyNotFound => Self::new(
                StatusCode::NOT_FOUND,
                "proxy_not_found",
                "proxy profile was not found",
            ),
            ConfigPublishError::ProxyProtected => Self::new(
                StatusCode::CONFLICT,
                "proxy_protected",
                "the built-in DIRECT proxy cannot be changed",
            ),
            ConfigPublishError::ProxyInUse => Self::new(
                StatusCode::CONFLICT,
                "proxy_in_use",
                "the global proxy cannot be deleted or disabled",
            ),
            ConfigPublishError::ProxyDisabled => Self::new(
                StatusCode::CONFLICT,
                "proxy_disabled",
                "a disabled proxy cannot be selected as global",
            ),
            ConfigPublishError::ProxyNameConflict => Self::new(
                StatusCode::CONFLICT,
                "proxy_name_conflict",
                "proxy name is already in use",
            ),
            ConfigPublishError::InvalidProxy(error) => {
                Self::new(StatusCode::BAD_REQUEST, "invalid_proxy", error.to_string())
            }
            ConfigPublishError::ProviderEndpointNotFound => Self::new(
                StatusCode::NOT_FOUND,
                "provider_endpoint_not_found",
                "provider endpoint was not found",
            ),
            ConfigPublishError::ProviderEndpointVersionConflict => Self::new(
                StatusCode::CONFLICT,
                "provider_endpoint_version_conflict",
                "provider endpoint changed; review the latest values before saving",
            ),
            ConfigPublishError::ProviderEndpointNameConflict => Self::new(
                StatusCode::CONFLICT,
                "provider_endpoint_name_conflict",
                "provider endpoint name is already in use",
            ),
            ConfigPublishError::InvalidProviderEndpoint(error) => Self::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_endpoint",
                error.to_string(),
            ),
            internal => {
                tracing::error!(error = ?internal, "configuration publish failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "configuration could not be published",
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

        (self.status, Json(body)).into_response()
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

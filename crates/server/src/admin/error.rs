use any2api_runtime::api::ConfigPublishError;
use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::CACHE_CONTROL},
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
            ConfigPublishError::ProxyReferenced => Self::new(
                StatusCode::CONFLICT,
                "proxy_referenced",
                "proxy profile is referenced by a provider credential",
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
            ConfigPublishError::ProviderEndpointInUse => Self::new(
                StatusCode::CONFLICT,
                "provider_endpoint_in_use",
                "provider endpoint is referenced by a provider credential or model route",
            ),
            ConfigPublishError::ProviderEndpointIdentityInUse => Self::new(
                StatusCode::CONFLICT,
                "provider_endpoint_identity_in_use",
                "provider and protocol cannot change while credentials or model routes exist",
            ),
            ConfigPublishError::InvalidProviderEndpoint(error) => Self::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_endpoint",
                error.to_string(),
            ),
            ConfigPublishError::ProviderCredentialNotFound => Self::provider_credential_not_found(),
            ConfigPublishError::ProviderCredentialVersionConflict => Self::new(
                StatusCode::CONFLICT,
                "provider_credential_version_conflict",
                "provider credential changed; review the latest values before saving",
            ),
            ConfigPublishError::ProviderCredentialSecretVersionConflict => Self::new(
                StatusCode::CONFLICT,
                "provider_credential_secret_version_conflict",
                "provider credential secret changed; refresh before rotating again",
            ),
            ConfigPublishError::ProviderCredentialLabelConflict => Self::new(
                StatusCode::CONFLICT,
                "provider_credential_label_conflict",
                "provider credential label is already in use for this endpoint",
            ),
            ConfigPublishError::InvalidProviderCredential(error) => Self::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_credential",
                error.to_string(),
            ),
            ConfigPublishError::InvalidProviderApiKey(error) => Self::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_api_key",
                error.to_string(),
            ),
            ConfigPublishError::GatewayApiKeyNotFound => Self::new(
                StatusCode::NOT_FOUND,
                "gateway_api_key_not_found",
                "gateway API Key was not found",
            ),
            ConfigPublishError::GatewayApiKeyVersionConflict => Self::new(
                StatusCode::CONFLICT,
                "gateway_api_key_version_conflict",
                "gateway API Key changed; review the latest values before saving",
            ),
            ConfigPublishError::GatewayApiKeyTokenVersionConflict => Self::new(
                StatusCode::CONFLICT,
                "gateway_api_key_token_version_conflict",
                "gateway API Key token changed; refresh before rotating again",
            ),
            ConfigPublishError::GatewayApiKeyNameConflict => Self::new(
                StatusCode::CONFLICT,
                "gateway_api_key_name_conflict",
                "gateway API Key name is already in use",
            ),
            ConfigPublishError::GatewayApiKeyRevoked => Self::new(
                StatusCode::CONFLICT,
                "gateway_api_key_revoked",
                "a revoked gateway API Key cannot be re-enabled or rotated",
            ),
            ConfigPublishError::InvalidGatewayApiKey(error) => Self::new(
                StatusCode::BAD_REQUEST,
                "invalid_gateway_api_key",
                error.to_string(),
            ),
            ConfigPublishError::ModelRouteNotFound => Self::model_route_not_found(),
            ConfigPublishError::ModelRouteVersionConflict => Self::new(
                StatusCode::CONFLICT,
                "model_route_version_conflict",
                "model route changed; review the latest values before saving",
            ),
            ConfigPublishError::ModelRouteNameConflict => Self::new(
                StatusCode::CONFLICT,
                "model_route_name_conflict",
                "public model is already in use for this ingress protocol",
            ),
            ConfigPublishError::RouteTargetIdentityConflict => Self::new(
                StatusCode::CONFLICT,
                "route_target_identity_conflict",
                "route target endpoint or upstream model cannot change under the same id",
            ),
            ConfigPublishError::InvalidModelRoute(error) => Self::new(
                StatusCode::BAD_REQUEST,
                "invalid_model_route",
                error.to_string(),
            ),
            ConfigPublishError::InvalidSetting(error) => Self::invalid_setting(error.to_string()),
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

        let mut response = (self.status, Json(body)).into_response();
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
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

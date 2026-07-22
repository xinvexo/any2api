use any2api_runtime::api::ConfigPublishError;
use axum::http::StatusCode;

use super::error::AdminApiError;

impl From<ConfigPublishError> for AdminApiError {
    fn from(error: ConfigPublishError) -> Self {
        match error {
            ConfigPublishError::ShuttingDown => AdminApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "server_shutting_down",
                "service is shutting down",
            ),
            ConfigPublishError::RevisionConflict { .. } => AdminApiError::new(
                StatusCode::CONFLICT,
                "revision_conflict",
                "configuration changed; refresh and try again",
            ),
            ConfigPublishError::ProxyNotFound => AdminApiError::new(
                StatusCode::NOT_FOUND,
                "proxy_not_found",
                "proxy profile was not found",
            ),
            ConfigPublishError::ProxyProtected => AdminApiError::new(
                StatusCode::CONFLICT,
                "proxy_protected",
                "the built-in DIRECT proxy cannot be changed",
            ),
            ConfigPublishError::ProxyInUse => AdminApiError::new(
                StatusCode::CONFLICT,
                "proxy_in_use",
                "the global proxy cannot be deleted or disabled",
            ),
            ConfigPublishError::ProxyReferenced => AdminApiError::new(
                StatusCode::CONFLICT,
                "proxy_referenced",
                "proxy profile is referenced by a provider credential",
            ),
            ConfigPublishError::ProxyDisabled => AdminApiError::new(
                StatusCode::CONFLICT,
                "proxy_disabled",
                "a disabled proxy cannot be selected as global",
            ),
            ConfigPublishError::ProxyNameConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "proxy_name_conflict",
                "proxy name is already in use",
            ),
            ConfigPublishError::InvalidProxy(error) => {
                AdminApiError::new(StatusCode::BAD_REQUEST, "invalid_proxy", error.to_string())
            }
            ConfigPublishError::InvalidProxyPassword(error) => AdminApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_proxy_password",
                error.to_string(),
            ),
            ConfigPublishError::ProviderEndpointNotFound => AdminApiError::new(
                StatusCode::NOT_FOUND,
                "provider_endpoint_not_found",
                "provider endpoint was not found",
            ),
            ConfigPublishError::ProviderEndpointVersionConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "provider_endpoint_version_conflict",
                "provider endpoint changed; review the latest values before saving",
            ),
            ConfigPublishError::ProviderEndpointNameConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "provider_endpoint_name_conflict",
                "provider endpoint name is already in use",
            ),
            ConfigPublishError::ProviderEndpointInUse => AdminApiError::new(
                StatusCode::CONFLICT,
                "provider_endpoint_in_use",
                "provider endpoint is referenced by a provider credential or model route",
            ),
            ConfigPublishError::ProviderEndpointIdentityInUse => AdminApiError::new(
                StatusCode::CONFLICT,
                "provider_endpoint_identity_in_use",
                "provider and protocol cannot change while credentials or model routes exist",
            ),
            ConfigPublishError::InvalidProviderEndpoint(error) => AdminApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_endpoint",
                error.to_string(),
            ),
            ConfigPublishError::ProviderCredentialNotFound => {
                AdminApiError::provider_credential_not_found()
            }
            ConfigPublishError::ProviderCredentialVersionConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "provider_credential_version_conflict",
                "provider credential changed; review the latest values before saving",
            ),
            ConfigPublishError::ProviderCredentialSecretVersionConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "provider_credential_secret_version_conflict",
                "provider credential secret changed; refresh before rotating again",
            ),
            ConfigPublishError::ProviderCredentialLabelConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "provider_credential_label_conflict",
                "provider credential label is already in use for this endpoint",
            ),
            ConfigPublishError::InvalidProviderCredential(error) => AdminApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_credential",
                error.to_string(),
            ),
            ConfigPublishError::InvalidProviderApiKey(error) => AdminApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_provider_api_key",
                error.to_string(),
            ),
            ConfigPublishError::GatewayApiKeyNotFound => AdminApiError::new(
                StatusCode::NOT_FOUND,
                "gateway_api_key_not_found",
                "gateway API Key was not found",
            ),
            ConfigPublishError::GatewayApiKeyVersionConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "gateway_api_key_version_conflict",
                "gateway API Key changed; review the latest values before saving",
            ),
            ConfigPublishError::GatewayApiKeyTokenVersionConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "gateway_api_key_token_version_conflict",
                "gateway API Key token changed; refresh before rotating again",
            ),
            ConfigPublishError::GatewayApiKeyNameConflict => AdminApiError::new(
                StatusCode::CONFLICT,
                "gateway_api_key_name_conflict",
                "gateway API Key name is already in use",
            ),
            ConfigPublishError::GatewayApiKeyRevoked => AdminApiError::new(
                StatusCode::CONFLICT,
                "gateway_api_key_revoked",
                "a revoked gateway API Key cannot be re-enabled or rotated",
            ),
            ConfigPublishError::InvalidGatewayApiKey(error) => AdminApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_gateway_api_key",
                error.to_string(),
            ),
            ConfigPublishError::InvalidModelRoute(error) => AdminApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_model_route",
                error.to_string(),
            ),
            ConfigPublishError::InvalidSetting(error) => {
                AdminApiError::invalid_setting(error.to_string())
            }
            internal => {
                tracing::error!(error = ?internal, "configuration publish failed");
                AdminApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "configuration could not be published",
                )
            }
        }
    }
}

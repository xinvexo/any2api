use any2api_domain::{
    ConfigRevision, GatewayApiKeyValidationError, ModelRouteValidationError,
    ProviderCredentialValidationError, ProviderEndpointValidationError, ProxyValidationError,
    SettingsValidationError,
};
use any2api_storage::api::{
    ProviderApiKeyValidationError, ProxyPasswordValidationError, StorageError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigPublishError {
    #[error("service is shutting down")]
    ShuttingDown,
    #[error("configuration revision conflict")]
    RevisionConflict {
        expected: ConfigRevision,
        actual: ConfigRevision,
    },
    #[error("configuration revision cannot be incremented")]
    RevisionOverflow,
    #[error("proxy profile was not found")]
    ProxyNotFound,
    #[error("the built-in DIRECT proxy cannot be changed")]
    ProxyProtected,
    #[error("proxy profile is currently selected as the global proxy")]
    ProxyInUse,
    #[error("proxy profile is referenced by a provider credential")]
    ProxyReferenced,
    #[error("disabled proxy profile cannot be selected as global")]
    ProxyDisabled,
    #[error("proxy name is already in use")]
    ProxyNameConflict,
    #[error("proxy configuration is invalid: {0}")]
    InvalidProxy(ProxyValidationError),
    #[error("invalid proxy password: {0}")]
    InvalidProxyPassword(ProxyPasswordValidationError),
    #[error("provider endpoint was not found")]
    ProviderEndpointNotFound,
    #[error("provider endpoint version conflict")]
    ProviderEndpointVersionConflict,
    #[error("provider endpoint name is already in use")]
    ProviderEndpointNameConflict,
    #[error("provider endpoint is referenced by a provider credential")]
    ProviderEndpointInUse,
    #[error("provider endpoint identity cannot change while credentials exist")]
    ProviderEndpointIdentityInUse,
    #[error("invalid provider endpoint: {0}")]
    InvalidProviderEndpoint(ProviderEndpointValidationError),
    #[error("provider credential was not found")]
    ProviderCredentialNotFound,
    #[error("provider credential version conflict")]
    ProviderCredentialVersionConflict,
    #[error("provider credential secret version conflict")]
    ProviderCredentialSecretVersionConflict,
    #[error("provider credential label is already in use for this endpoint")]
    ProviderCredentialLabelConflict,
    #[error("invalid provider credential: {0}")]
    InvalidProviderCredential(ProviderCredentialValidationError),
    #[error("invalid provider API Key: {0}")]
    InvalidProviderApiKey(ProviderApiKeyValidationError),
    #[error("gateway API Key was not found")]
    GatewayApiKeyNotFound,
    #[error("gateway API Key version conflict")]
    GatewayApiKeyVersionConflict,
    #[error("gateway API Key token version conflict")]
    GatewayApiKeyTokenVersionConflict,
    #[error("gateway API Key name is already in use")]
    GatewayApiKeyNameConflict,
    #[error("gateway API Key was revoked")]
    GatewayApiKeyRevoked,
    #[error("invalid gateway API Key configuration: {0}")]
    InvalidGatewayApiKey(GatewayApiKeyValidationError),
    #[error("gateway API Key token generation failed")]
    GatewayApiKeyTokenGeneration,
    #[error("model route was not found")]
    ModelRouteNotFound,
    #[error("model route version conflict")]
    ModelRouteVersionConflict,
    #[error("public model is already in use for this ingress protocol")]
    ModelRouteNameConflict,
    #[error("route target identity cannot change under the same id")]
    RouteTargetIdentityConflict,
    #[error("invalid model route: {0}")]
    InvalidModelRoute(ModelRouteValidationError),
    #[error("invalid setting value: {0}")]
    InvalidSetting(SettingsValidationError),
    #[error("configuration storage failed")]
    Internal(#[source] StorageError),
}

impl From<StorageError> for ConfigPublishError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::RevisionConflict { expected, actual } => {
                Self::RevisionConflict { expected, actual }
            }
            StorageError::RevisionOverflow => Self::RevisionOverflow,
            StorageError::ProxyNotFound(_) => Self::ProxyNotFound,
            StorageError::ProxyProtected => Self::ProxyProtected,
            StorageError::ProxyInUse => Self::ProxyInUse,
            StorageError::ProxyReferenced => Self::ProxyReferenced,
            StorageError::ProxyDisabled => Self::ProxyDisabled,
            StorageError::ProxyNameConflict => Self::ProxyNameConflict,
            StorageError::ProxyValidation(error) => Self::InvalidProxy(error),
            StorageError::ProxyPasswordValidation(error) => Self::InvalidProxyPassword(error),
            StorageError::ProviderEndpointNotFound(_) => Self::ProviderEndpointNotFound,
            StorageError::ProviderEndpointVersionConflict { .. } => {
                Self::ProviderEndpointVersionConflict
            }
            StorageError::ProviderEndpointNameConflict => Self::ProviderEndpointNameConflict,
            StorageError::ProviderEndpointInUse => Self::ProviderEndpointInUse,
            StorageError::ProviderEndpointIdentityInUse => Self::ProviderEndpointIdentityInUse,
            StorageError::ProviderEndpointValidation(error) => Self::InvalidProviderEndpoint(error),
            StorageError::ProviderCredentialNotFound(_) => Self::ProviderCredentialNotFound,
            StorageError::ProviderCredentialVersionConflict { .. } => {
                Self::ProviderCredentialVersionConflict
            }
            StorageError::ProviderCredentialSecretVersionConflict { .. } => {
                Self::ProviderCredentialSecretVersionConflict
            }
            StorageError::ProviderCredentialLabelConflict => Self::ProviderCredentialLabelConflict,
            StorageError::ProviderCredentialValidation(error) => {
                Self::InvalidProviderCredential(error)
            }
            StorageError::ProviderApiKeyValidation(error) => Self::InvalidProviderApiKey(error),
            StorageError::GatewayApiKeyNotFound(_) => Self::GatewayApiKeyNotFound,
            StorageError::GatewayApiKeyVersionConflict { .. } => Self::GatewayApiKeyVersionConflict,
            StorageError::GatewayApiKeyTokenVersionConflict { .. } => {
                Self::GatewayApiKeyTokenVersionConflict
            }
            StorageError::GatewayApiKeyNameConflict => Self::GatewayApiKeyNameConflict,
            StorageError::GatewayApiKeyRevoked => Self::GatewayApiKeyRevoked,
            StorageError::GatewayApiKeyValidation(error) => Self::InvalidGatewayApiKey(error),
            StorageError::ModelRouteNotFound(_) => Self::ModelRouteNotFound,
            StorageError::ModelRouteVersionConflict { .. } => Self::ModelRouteVersionConflict,
            StorageError::ModelRouteNameConflict => Self::ModelRouteNameConflict,
            StorageError::RouteTargetIdentityConflict => Self::RouteTargetIdentityConflict,
            StorageError::ModelRouteValidation(error) => Self::InvalidModelRoute(error),
            StorageError::SettingsValidation(error) => Self::InvalidSetting(error),
            other => Self::Internal(other),
        }
    }
}

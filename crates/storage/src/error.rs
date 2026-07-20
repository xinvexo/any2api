use std::path::PathBuf;

use any2api_domain::{
    ConfigRevision, CredentialId, GatewayApiKeyId, GatewayApiKeyValidationError, ModelRouteId,
    ModelRouteValidationError, ProviderCredentialValidationError, ProviderEndpointId,
    ProviderEndpointValidationError, ProxyProfileId, ProxyValidationError, SettingsValidationError,
};
use thiserror::Error;

use crate::provider_api_key::ProviderApiKeyValidationError;
use crate::vault::SecretVaultError;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to create database directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("sqlite operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("sqlite migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("stored configuration revision is invalid: {0}")]
    InvalidRevision(i64),
    #[error("configuration revision conflict")]
    RevisionConflict {
        expected: ConfigRevision,
        actual: ConfigRevision,
    },
    #[error("configuration revision cannot be incremented")]
    RevisionOverflow,
    #[error("proxy profile was not found")]
    ProxyNotFound(ProxyProfileId),
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
    ProxyValidation(#[from] ProxyValidationError),
    #[error("provider endpoint was not found")]
    ProviderEndpointNotFound(ProviderEndpointId),
    #[error("provider endpoint version conflict")]
    ProviderEndpointVersionConflict { expected: u64, actual: u64 },
    #[error("provider endpoint name is already in use")]
    ProviderEndpointNameConflict,
    #[error("provider endpoint is referenced by a provider credential")]
    ProviderEndpointInUse,
    #[error("provider endpoint identity cannot change while credentials exist")]
    ProviderEndpointIdentityInUse,
    #[error("provider endpoint configuration is invalid: {0}")]
    ProviderEndpointValidation(#[from] ProviderEndpointValidationError),
    #[error("model route was not found")]
    ModelRouteNotFound(ModelRouteId),
    #[error("model route version conflict")]
    ModelRouteVersionConflict { expected: u64, actual: u64 },
    #[error("public model is already in use for this ingress protocol")]
    ModelRouteNameConflict,
    #[error("route target identity cannot change under the same id")]
    RouteTargetIdentityConflict,
    #[error("model route configuration is invalid: {0}")]
    ModelRouteValidation(#[from] ModelRouteValidationError),
    #[error("provider credential was not found")]
    ProviderCredentialNotFound(CredentialId),
    #[error("provider credential version conflict")]
    ProviderCredentialVersionConflict { expected: u64, actual: u64 },
    #[error("provider credential secret version conflict")]
    ProviderCredentialSecretVersionConflict { expected: u64, actual: u64 },
    #[error("provider credential label is already in use for this endpoint")]
    ProviderCredentialLabelConflict,
    #[error("provider credential configuration is invalid: {0}")]
    ProviderCredentialValidation(#[from] ProviderCredentialValidationError),
    #[error("provider API Key is invalid: {0}")]
    ProviderApiKeyValidation(#[from] ProviderApiKeyValidationError),
    #[error("gateway API Key was not found")]
    GatewayApiKeyNotFound(GatewayApiKeyId),
    #[error("gateway API Key version conflict")]
    GatewayApiKeyVersionConflict { expected: u64, actual: u64 },
    #[error("gateway API Key token version conflict")]
    GatewayApiKeyTokenVersionConflict { expected: u64, actual: u64 },
    #[error("gateway API Key name is already in use")]
    GatewayApiKeyNameConflict,
    #[error("gateway API Key was revoked")]
    GatewayApiKeyRevoked,
    #[error("gateway API Key configuration is invalid: {0}")]
    GatewayApiKeyValidation(#[from] GatewayApiKeyValidationError),
    #[error("generated gateway API Key token is invalid")]
    InvalidGatewayApiKeyToken,
    #[error("stored gateway API Key hash key does not match the current vault")]
    GatewayApiKeyHashKeyMismatch,
    #[error("stored configuration is invalid")]
    CorruptConfiguration,
    #[error("stored request telemetry is invalid")]
    CorruptTelemetry,
    #[error("setting value is invalid: {0}")]
    SettingsValidation(#[from] SettingsValidationError),
    #[error("secret vault initialization failed: {0}")]
    SecretVault(#[from] SecretVaultError),
}

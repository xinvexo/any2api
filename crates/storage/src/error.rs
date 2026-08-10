use std::{fmt, path::PathBuf};

use any2api_domain::{
    ConfigRevision, CredentialId, GatewayApiKeyId, GatewayApiKeyValidationError,
    ModelRouteValidationError, OAuthAccountId, OAuthAccountValidationError,
    ProviderCredentialValidationError, ProviderEndpointId, ProviderEndpointValidationError,
    ProxyProfileId, ProxyValidationError, SettingsValidationError,
};
use thiserror::Error;

use crate::oauth_account::OAuthAccountDocumentValidationError;
use crate::provider::ProviderApiKeyValidationError;
use crate::proxy::ProxyPasswordValidationError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationWriteComponent {
    Revision,
    GatewayApiKeys,
    ProxyProfiles,
    OAuthAccounts,
    ProviderCredentials,
    ProviderEndpoints,
    ModelRoutes,
    Settings,
}

impl ConfigurationWriteComponent {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 8] = [
        Self::Revision,
        Self::GatewayApiKeys,
        Self::ProxyProfiles,
        Self::OAuthAccounts,
        Self::ProviderCredentials,
        Self::ProviderEndpoints,
        Self::ModelRoutes,
        Self::Settings,
    ];
}

impl fmt::Display for ConfigurationWriteComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Revision => "revision",
            Self::GatewayApiKeys => "gateway API Keys",
            Self::ProxyProfiles => "proxy profiles",
            Self::OAuthAccounts => "OAuth accounts",
            Self::ProviderCredentials => "provider credentials",
            Self::ProviderEndpoints => "provider endpoints",
            Self::ModelRoutes => "model routes",
            Self::Settings => "settings",
        })
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to protect local data path {path}: {source}")]
    ProtectLocalData {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("sqlite operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("configuration commit outcome is indeterminate")]
    IndeterminateConfigurationCommit {
        #[source]
        source: sqlx::Error,
    },
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
    #[error("persisted configuration does not match prepared {0} after write")]
    ConfigurationWriteMismatch(ConfigurationWriteComponent),
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
    #[error("proxy password is invalid: {0}")]
    ProxyPasswordValidation(#[from] ProxyPasswordValidationError),
    #[error("provider endpoint was not found")]
    ProviderEndpointNotFound(ProviderEndpointId),
    #[error("provider endpoint version conflict")]
    ProviderEndpointVersionConflict { expected: u64, actual: u64 },
    #[error("provider endpoint name is already in use")]
    ProviderEndpointNameConflict,
    #[error("provider endpoint configuration is invalid: {0}")]
    ProviderEndpointValidation(#[from] ProviderEndpointValidationError),
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
    #[error("OAuth account was not found")]
    OAuthAccountNotFound(OAuthAccountId),
    #[error("OAuth account version conflict")]
    OAuthAccountVersionConflict { expected: u64, actual: u64 },
    #[error("OAuth account token version conflict")]
    OAuthAccountTokenVersionConflict { expected: u64, actual: u64 },
    #[error("OAuth account label is already in use for this provider")]
    OAuthAccountLabelConflict,
    #[error("OAuth account configuration is invalid: {0}")]
    OAuthAccountValidation(#[from] OAuthAccountValidationError),
    #[error("OAuth account JSON is invalid: {0}")]
    OAuthAccountDocumentValidation(#[from] OAuthAccountDocumentValidationError),
    #[error("gateway API Key was not found")]
    GatewayApiKeyNotFound(GatewayApiKeyId),
    #[error("gateway API Key version conflict")]
    GatewayApiKeyVersionConflict { expected: u64, actual: u64 },
    #[error("gateway API Key token version conflict")]
    GatewayApiKeyTokenVersionConflict { expected: u64, actual: u64 },
    #[error("gateway API Key name is already in use")]
    GatewayApiKeyNameConflict,
    #[error("gateway API Key configuration is invalid: {0}")]
    GatewayApiKeyValidation(#[from] GatewayApiKeyValidationError),
    #[error("generated gateway API Key token is invalid")]
    InvalidGatewayApiKeyToken,
    #[error("stored configuration is invalid")]
    CorruptConfiguration,
    #[error("stored request telemetry is invalid")]
    CorruptTelemetry,
    #[error("stored OAuth quota snapshot is invalid")]
    CorruptOAuthQuotaSnapshot,
    #[error("setting value is invalid: {0}")]
    SettingsValidation(#[from] SettingsValidationError),
}

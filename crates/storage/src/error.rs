use std::path::PathBuf;

use any2api_domain::{
    ConfigRevision, ProviderEndpointId, ProviderEndpointValidationError, ProxyProfileId,
    ProxyValidationError,
};
use thiserror::Error;

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
    #[error("provider endpoint configuration is invalid: {0}")]
    ProviderEndpointValidation(#[from] ProviderEndpointValidationError),
    #[error("stored proxy configuration is invalid")]
    CorruptConfiguration,
    #[error("secret vault initialization failed: {0}")]
    SecretVault(#[from] SecretVaultError),
}

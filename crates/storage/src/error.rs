use std::path::PathBuf;

use any2api_domain::{ConfigRevision, ProxyProfileId, ProxyValidationError};
use thiserror::Error;

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
    #[error("stored proxy configuration is invalid")]
    CorruptConfiguration,
}

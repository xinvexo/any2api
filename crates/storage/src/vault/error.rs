use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretVaultError {
    #[error("failed to create master key directory {path}: {source}")]
    CreateMasterKeyDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("secret vault master key file is missing: {path}")]
    MasterKeyMissing { path: PathBuf },
    #[error("secret vault master key file must be separate from the SQLite database")]
    MasterKeyPathConflictsWithDatabase,
    #[error("failed to read secret vault master key file {path}: {source}")]
    ReadMasterKey {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create secret vault master key file {path}: {source}")]
    CreateMasterKey {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write secret vault master key file {path}: {source}")]
    WriteMasterKey {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("secret vault master key file has unsafe permissions: {path}")]
    UnsafeMasterKeyPermissions { path: PathBuf },
    #[error("secret vault master key file format is invalid")]
    InvalidMasterKeyFormat,
    #[error("secret vault master key file version is unsupported")]
    UnsupportedMasterKeyVersion,
    #[error("secret vault master key file algorithm is unsupported")]
    UnsupportedMasterKeyAlgorithm,
    #[error("secure random generation failed")]
    RandomGeneration,
    #[error("secret envelope version is unsupported")]
    UnsupportedEnvelopeVersion,
    #[error("secret envelope algorithm is unsupported")]
    UnsupportedEnvelopeAlgorithm,
    #[error("secret envelope AAD version is unsupported")]
    UnsupportedAadVersion,
    #[error("secret envelope is invalid")]
    InvalidEnvelope,
    #[error("secret vault master key does not match the initialized database")]
    KeyMismatch,
    #[error("secret encryption failed")]
    EncryptionFailed,
    #[error("secret authentication failed")]
    AuthenticationFailed,
    #[error("secret vault database operation failed")]
    Database(#[from] sqlx::Error),
}

pub use crate::error::StorageError;
pub use crate::proxy_repository::{ConfigurationRepository, StoredConfiguration};
pub use crate::sqlite::SqliteStore;
pub use crate::vault::{
    SecretAlgorithm, SecretBytes, SecretContext, SecretEnvelope, SecretVault, SecretVaultError,
};

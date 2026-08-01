use any2api_domain::ConfigRevision;
use async_trait::async_trait;

use crate::error::StorageError;

use super::{ConfigurationMutation, PreparedConfiguration, StoredConfiguration};

#[async_trait]
pub trait ConfigurationRepository: Send + Sync {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError>;

    async fn prepare_configuration(
        &self,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
    ) -> Result<PreparedConfiguration, StorageError>;
}

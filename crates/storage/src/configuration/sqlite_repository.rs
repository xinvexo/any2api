use any2api_domain::ConfigRevision;
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    ConfigurationMutation, ConfigurationRepository, PreparedConfiguration, StoredConfiguration,
    load_configuration_from,
};

#[async_trait]
impl ConfigurationRepository for SqliteStore {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let configuration = load_configuration_from(&mut transaction).await?;
        transaction.commit().await?;
        Ok(configuration)
    }

    async fn prepare_configuration(
        &self,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
    ) -> Result<PreparedConfiguration, StorageError> {
        self.prepare_configuration_mutation(expected, mutation)
            .await
    }
}

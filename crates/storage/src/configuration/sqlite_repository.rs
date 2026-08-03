use any2api_domain::ConfigRevision;
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    ConfigurationCandidateCompiler, ConfigurationMutation, ConfigurationRepository,
    ConfigurationTransactionOutcome, ConfigurationTransactionRepository, StoredConfiguration,
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
}

#[async_trait]
impl<Accepted, Rejected> ConfigurationTransactionRepository<Accepted, Rejected> for SqliteStore
where
    Accepted: Send + 'static,
    Rejected: Send + 'static,
{
    async fn transact_configuration(
        &self,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
        compiler: ConfigurationCandidateCompiler<Accepted, Rejected>,
    ) -> Result<ConfigurationTransactionOutcome<Accepted, Rejected>, StorageError> {
        self.transact_configuration_mutation(expected, mutation, compiler)
            .await
    }
}

use any2api_domain::ConfigRevision;
use async_trait::async_trait;

use crate::error::StorageError;

use super::{
    ConfigurationCandidateCompiler, ConfigurationMutation, ConfigurationTransactionOutcome,
    StoredConfiguration,
};

#[async_trait]
pub trait ConfigurationRepository: Send + Sync {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError>;
}

#[async_trait]
pub trait ConfigurationTransactionRepository<Accepted, Rejected>: ConfigurationRepository
where
    Accepted: Send + 'static,
    Rejected: Send + 'static,
{
    async fn transact_configuration(
        &self,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
        compiler: ConfigurationCandidateCompiler<Accepted, Rejected>,
    ) -> Result<ConfigurationTransactionOutcome<Accepted, Rejected>, StorageError>;
}

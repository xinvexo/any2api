mod dispatch;

use any2api_domain::ConfigRevision;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{ConfigurationMutation, StoredConfiguration};

pub type ConfigurationCandidateCompiler<Accepted, Rejected> =
    Box<dyn FnOnce(StoredConfiguration) -> Result<Accepted, Rejected> + Send + 'static>;

pub enum ConfigurationTransactionOutcome<Accepted, Rejected> {
    NoChange,
    Committed(Accepted),
    Rejected(Rejected),
}

impl SqliteStore {
    pub(crate) async fn transact_configuration_mutation<Accepted, Rejected>(
        &self,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
        compiler: ConfigurationCandidateCompiler<Accepted, Rejected>,
    ) -> Result<ConfigurationTransactionOutcome<Accepted, Rejected>, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (candidate, changed) =
            dispatch::execute_mutation(&mut transaction, expected, mutation).await?;
        if !changed {
            transaction.rollback().await?;
            return Ok(ConfigurationTransactionOutcome::NoChange);
        }
        match compiler(candidate) {
            Ok(accepted) => {
                transaction.commit().await?;
                Ok(ConfigurationTransactionOutcome::Committed(accepted))
            }
            Err(rejected) => {
                transaction.rollback().await?;
                Ok(ConfigurationTransactionOutcome::Rejected(rejected))
            }
        }
    }
}

#[cfg(test)]
mod tests;

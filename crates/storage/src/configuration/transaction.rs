mod dispatch;

use any2api_domain::ConfigRevision;

use crate::{
    error::StorageError,
    sqlite::SqliteStore,
    sqlite_commit::{self, SqliteCommitError},
};

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
        let mut transaction = self.begin_write().await?;
        let (candidate, changed) =
            dispatch::execute_mutation(&mut transaction, expected, mutation).await?;
        if !changed {
            transaction.rollback().await?;
            return Ok(ConfigurationTransactionOutcome::NoChange);
        }
        // Compilation is CPU-bound and runs while the write transaction is
        // open; hand the worker thread back to the runtime so other async
        // tasks (telemetry flushes in particular) keep making progress.
        // block_in_place is unavailable on current-thread runtimes (tests).
        let handle = tokio::runtime::Handle::current();
        let compiled = if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
            tokio::task::block_in_place(|| compiler(candidate))
        } else {
            compiler(candidate)
        };
        match compiled {
            Ok(accepted) => {
                commit_configuration_transaction(transaction).await?;
                Ok(ConfigurationTransactionOutcome::Committed(accepted))
            }
            Err(rejected) => {
                transaction.rollback().await?;
                Ok(ConfigurationTransactionOutcome::Rejected(rejected))
            }
        }
    }
}

async fn commit_configuration_transaction(
    transaction: sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<(), StorageError> {
    sqlite_commit::commit(transaction)
        .await
        .map_err(|error| match error {
            SqliteCommitError::Determinate(source) => StorageError::Database(source),
            SqliteCommitError::Indeterminate(source) => {
                StorageError::IndeterminateConfigurationCommit { source }
            }
        })
}

#[cfg(test)]
pub(crate) use crate::sqlite_commit::{
    SQLITE_PRIMARY_CODE_MASK, commit_error_is_indeterminate, sqlite_primary_code_is_indeterminate,
};

#[cfg(test)]
mod tests;

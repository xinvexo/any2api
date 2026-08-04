mod dispatch;

use any2api_domain::ConfigRevision;
use sqlx::{Sqlite, Transaction};

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

const SQLITE_PRIMARY_CODE_MASK: i32 = 0xff;
const SQLITE_IOERR_PRIMARY_CODE: i32 = 10;

async fn commit_configuration_transaction(
    transaction: Transaction<'_, Sqlite>,
) -> Result<(), StorageError> {
    transaction.commit().await.map_err(|source| {
        if commit_error_is_indeterminate(&source) {
            StorageError::IndeterminateConfigurationCommit { source }
        } else {
            StorageError::Database(source)
        }
    })
}

fn commit_error_is_indeterminate(error: &sqlx::Error) -> bool {
    // SQLite can report IOERR from COMMIT_PHASETWO after the WAL commit is visible. A missing
    // worker acknowledgement is equally unsafe; neither case may be returned as a normal error.
    let sqlx::Error::Database(database) = error else {
        return true;
    };
    database
        .code()
        .and_then(|code| code.parse::<i32>().ok())
        .is_none_or(|code| sqlite_primary_code_is_indeterminate(code & SQLITE_PRIMARY_CODE_MASK))
}

const fn sqlite_primary_code_is_indeterminate(code: i32) -> bool {
    code == SQLITE_IOERR_PRIMARY_CODE
}

#[cfg(test)]
mod tests;

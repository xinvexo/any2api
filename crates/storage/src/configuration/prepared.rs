mod dispatch;

use any2api_domain::ConfigRevision;
use sqlx::{Sqlite, Transaction};

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{ConfigurationMutation, StoredConfiguration};

pub struct PreparedConfiguration {
    transaction: Transaction<'static, Sqlite>,
    candidate: StoredConfiguration,
    changed: bool,
}

impl PreparedConfiguration {
    #[must_use]
    pub const fn changed(&self) -> bool {
        self.changed
    }

    #[must_use]
    pub const fn candidate(&self) -> &StoredConfiguration {
        &self.candidate
    }

    #[must_use]
    pub fn into_parts(self) -> (StoredConfiguration, ConfigurationCommit) {
        (
            self.candidate,
            ConfigurationCommit {
                transaction: self.transaction,
                changed: self.changed,
            },
        )
    }
}

pub struct ConfigurationCommit {
    transaction: Transaction<'static, Sqlite>,
    changed: bool,
}

impl ConfigurationCommit {
    pub async fn finish(self) -> Result<(), StorageError> {
        if self.changed {
            self.transaction.commit().await?;
        } else {
            self.transaction.rollback().await?;
        }
        Ok(())
    }

    pub async fn rollback(self) -> Result<(), StorageError> {
        self.transaction.rollback().await?;
        Ok(())
    }
}

impl SqliteStore {
    pub(crate) async fn prepare_configuration_mutation(
        &self,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
    ) -> Result<PreparedConfiguration, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (candidate, changed) =
            dispatch::execute_mutation(&mut transaction, expected, mutation).await?;
        Ok(PreparedConfiguration {
            transaction,
            candidate,
            changed,
        })
    }
}

#[cfg(test)]
mod tests;

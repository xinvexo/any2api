use std::convert::Infallible;

use any2api_domain::ConfigRevision;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    ConfigurationMutation, ConfigurationRepository, ConfigurationTransactionOutcome,
    ConfigurationTransactionRepository, StoredConfiguration,
};

pub(crate) async fn commit_configuration(
    store: &SqliteStore,
    expected: ConfigRevision,
    mutation: ConfigurationMutation,
) -> Result<StoredConfiguration, StorageError> {
    let outcome = <SqliteStore as ConfigurationTransactionRepository<
        StoredConfiguration,
        Infallible,
    >>::transact_configuration(store, expected, mutation, Box::new(Ok))
    .await?;
    match outcome {
        ConfigurationTransactionOutcome::NoChange => store.load_configuration().await,
        ConfigurationTransactionOutcome::Committed(configuration) => Ok(configuration),
        ConfigurationTransactionOutcome::Rejected(never) => match never {},
    }
}

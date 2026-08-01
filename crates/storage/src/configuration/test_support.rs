use any2api_domain::ConfigRevision;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{ConfigurationMutation, ConfigurationRepository, StoredConfiguration};

pub(crate) async fn commit_configuration(
    store: &SqliteStore,
    expected: ConfigRevision,
    mutation: ConfigurationMutation,
) -> Result<StoredConfiguration, StorageError> {
    let prepared = store.prepare_configuration(expected, mutation).await?;
    let (candidate, commit) = prepared.into_parts();
    commit.finish().await?;
    Ok(candidate)
}

use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    error::StorageError,
    provider_endpoint_mutation::{ProviderEndpointMutation, prepare_provider_endpoint_mutation},
    provider_endpoint_rows::execute_provider_endpoint_change,
    proxy_repository::{StoredConfiguration, bump_revision},
    proxy_rows::load_configuration_from,
    sqlite::SqliteStore,
};

impl SqliteStore {
    pub(crate) async fn mutate_provider_endpoint(
        &self,
        expected: ConfigRevision,
        mutation: ProviderEndpointMutation,
    ) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (configuration, changed) =
            mutate_connection(&mut transaction, expected, mutation).await?;
        if changed {
            transaction.commit().await?;
        } else {
            transaction.rollback().await?;
        }
        Ok(configuration)
    }
}

async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: ProviderEndpointMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) =
        prepare_provider_endpoint_mutation(current.provider_endpoints(), mutation)?
    else {
        return Ok((current, false));
    };
    execute_provider_endpoint_change(connection, prepared.change()).await?;
    let revision = bump_revision(connection, expected).await?;
    Ok((
        StoredConfiguration::new(
            revision,
            current.proxies().clone(),
            prepared.into_configuration(),
        ),
        true,
    ))
}

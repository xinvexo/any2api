use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    provider_credential_writes::bump_endpoint_credential_generations,
    provider_endpoint_mutation::{ProviderEndpointMutation, prepare_provider_endpoint_mutation},
    provider_endpoint_rows::execute_provider_endpoint_change,
    proxy_repository::bump_revision,
    proxy_rows::load_configuration_from,
    sqlite::SqliteStore,
    vault::SecretVault,
};

impl SqliteStore {
    pub(crate) async fn mutate_provider_endpoint(
        &self,
        expected: ConfigRevision,
        mutation: ProviderEndpointMutation,
    ) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (configuration, changed) =
            mutate_connection(&mut transaction, self.secret_vault(), expected, mutation).await?;
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
    vault: &SecretVault,
    expected: ConfigRevision,
    mutation: ProviderEndpointMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) = prepare_provider_endpoint_mutation(
        current.provider_endpoints(),
        current.provider_credentials(),
        current.proxies(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    execute_provider_endpoint_change(connection, prepared.change()).await?;
    if prepared.bump_credential_generations() {
        bump_endpoint_credential_generations(connection, prepared.endpoint_id()).await?;
    }
    let revision = bump_revision(connection, expected).await?;
    let (provider_endpoints, provider_credentials) = prepared.into_configurations();
    Ok((
        StoredConfiguration::new(
            revision,
            current.proxies().clone(),
            provider_endpoints,
            provider_credentials,
        ),
        true,
    ))
}

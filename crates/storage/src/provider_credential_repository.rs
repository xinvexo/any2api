use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    provider_credential_mutation::{
        ProviderCredentialMutation, prepare_provider_credential_mutation,
    },
    provider_credential_writes::execute_provider_credential_change,
    proxy_repository::bump_revision,
    proxy_rows::load_configuration_from,
    sqlite::SqliteStore,
    vault::SecretVault,
};

impl SqliteStore {
    pub(crate) async fn mutate_provider_credential(
        &self,
        expected: ConfigRevision,
        mutation: ProviderCredentialMutation,
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
    mutation: ProviderCredentialMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) = prepare_provider_credential_mutation(
        current.provider_credentials(),
        current.provider_endpoints(),
        current.proxies(),
        vault,
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    execute_provider_credential_change(connection, prepared.change()).await?;
    let revision = bump_revision(connection, expected).await?;
    Ok((
        StoredConfiguration::new(
            revision,
            current.proxies().clone(),
            current.provider_endpoints().clone(),
            prepared.into_configuration(),
        ),
        true,
    ))
}

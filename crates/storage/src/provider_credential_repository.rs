use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    model_route_replacement::replace_model_routes,
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
    let expected_model_routes = prepared.model_routes().cloned();
    if let Some(model_routes) = expected_model_routes.as_ref() {
        replace_model_routes(connection, model_routes).await?;
    }
    let expected_credentials = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.provider_credentials(), &expected_credentials);
    if let Some(expected_model_routes) = expected_model_routes {
        assert_eq!(configuration.model_routes(), &expected_model_routes);
    }
    Ok((configuration, true))
}

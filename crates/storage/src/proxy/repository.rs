use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::{StoredConfiguration, bump_revision, load_configuration_from},
    error::StorageError,
    sqlite::SqliteStore,
};

use super::{
    mutation::{ProxyMutation, prepare_mutation},
    writes::execute_change,
};

impl SqliteStore {
    pub(crate) async fn mutate_proxy(
        &self,
        expected: ConfigRevision,
        mutation: ProxyMutation,
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
    vault: &crate::vault::SecretVault,
    expected: ConfigRevision,
    mutation: ProxyMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) =
        prepare_mutation(current.proxies(), current.provider_credentials(), mutation)?
    else {
        return Ok((current, false));
    };
    execute_change(connection, prepared.change()).await?;
    let expected_proxies = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.proxies(), &expected_proxies);
    Ok((configuration, true))
}

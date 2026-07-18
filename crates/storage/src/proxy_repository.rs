use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    proxy_mutation::{ProxyMutation, prepare_mutation},
    proxy_rows::{execute_change, load_configuration_from},
    sqlite::SqliteStore,
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
    let revision = bump_revision(connection, expected).await?;
    Ok((
        StoredConfiguration::new(
            revision,
            prepared.into_configuration(),
            current.provider_endpoints().clone(),
            current.provider_credentials().clone(),
        ),
        true,
    ))
}

pub(crate) async fn bump_revision(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
) -> Result<ConfigRevision, StorageError> {
    let next: Option<i64> = sqlx::query_scalar(
        "UPDATE config_state \
         SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP \
         WHERE singleton_id = 1 AND revision = ? AND revision < ? \
         RETURNING revision",
    )
    .bind(i64::try_from(expected.get()).map_err(|_| StorageError::RevisionOverflow)?)
    .bind(i64::MAX)
    .fetch_optional(connection)
    .await?;
    let next = next.ok_or(StorageError::RevisionOverflow)?;
    let next = u64::try_from(next).map_err(|_| StorageError::InvalidRevision(next))?;
    ConfigRevision::new(next).map_err(|_| StorageError::InvalidRevision(next as i64))
}

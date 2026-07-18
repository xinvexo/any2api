use any2api_domain::{ConfigRevision, ModelRouteDraft, ModelRouteId};
use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    model_route_mutation::{ModelRouteMutation, prepare_model_route_mutation},
    model_route_rows::execute_model_route_change,
    proxy_repository::bump_revision,
    proxy_rows::load_configuration_from,
    sqlite::SqliteStore,
    vault::SecretVault,
};

#[async_trait]
pub trait ModelRouteRepository: Send + Sync {
    async fn create_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        draft: ModelRouteDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn update_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        expected_config_version: u64,
        draft: ModelRouteDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn delete_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError>;
}

#[async_trait]
impl ModelRouteRepository for SqliteStore {
    async fn create_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        draft: ModelRouteDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_model_route(expected, ModelRouteMutation::Create { id, draft })
            .await
    }

    async fn update_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        expected_config_version: u64,
        draft: ModelRouteDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_model_route(
            expected,
            ModelRouteMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    async fn delete_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_model_route(
            expected,
            ModelRouteMutation::Delete {
                id,
                expected_config_version,
            },
        )
        .await
    }
}

impl SqliteStore {
    async fn mutate_model_route(
        &self,
        expected: ConfigRevision,
        mutation: ModelRouteMutation,
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
    mutation: ModelRouteMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) = prepare_model_route_mutation(
        current.model_routes(),
        current.provider_endpoints(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    execute_model_route_change(connection, prepared.change()).await?;
    let expected_routes = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.model_routes(), &expected_routes);
    Ok((configuration, true))
}

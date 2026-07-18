use any2api_domain::{
    ConfigRevision, ProviderEndpointConfiguration, ProviderEndpointDraft, ProviderEndpointId,
    ProxyConfiguration, ProxyDraft, ProxyProfileId,
};
use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::{
    error::StorageError,
    provider_endpoint_mutation::ProviderEndpointMutation,
    proxy_mutation::{ProxyMutation, prepare_mutation},
    proxy_rows::{execute_change, load_configuration_from},
    sqlite::SqliteStore,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredConfiguration {
    revision: ConfigRevision,
    proxies: ProxyConfiguration,
    provider_endpoints: ProviderEndpointConfiguration,
}

impl StoredConfiguration {
    #[must_use]
    pub const fn new(
        revision: ConfigRevision,
        proxies: ProxyConfiguration,
        provider_endpoints: ProviderEndpointConfiguration,
    ) -> Self {
        Self {
            revision,
            proxies,
            provider_endpoints,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    #[must_use]
    pub const fn proxies(&self) -> &ProxyConfiguration {
        &self.proxies
    }

    #[must_use]
    pub const fn provider_endpoints(&self) -> &ProviderEndpointConfiguration {
        &self.provider_endpoints
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        ConfigRevision,
        ProxyConfiguration,
        ProviderEndpointConfiguration,
    ) {
        (self.revision, self.proxies, self.provider_endpoints)
    }
}

#[async_trait]
pub trait ConfigurationRepository: Send + Sync {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError>;

    async fn create_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn update_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn delete_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn set_global_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn create_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn update_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
        expected_config_version: u64,
        draft: ProviderEndpointDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn delete_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
    ) -> Result<StoredConfiguration, StorageError>;
}

#[async_trait]
impl ConfigurationRepository for SqliteStore {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let configuration = load_configuration_from(&mut transaction).await?;
        transaction.commit().await?;
        Ok(configuration)
    }

    async fn create_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_proxy(expected, ProxyMutation::Create { id, draft })
            .await
    }

    async fn update_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_proxy(expected, ProxyMutation::Update { id, draft })
            .await
    }

    async fn delete_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_proxy(expected, ProxyMutation::Delete { id })
            .await
    }

    async fn set_global_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_proxy(expected, ProxyMutation::SetGlobal { id })
            .await
    }

    async fn create_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_endpoint(expected, ProviderEndpointMutation::Create { id, draft })
            .await
    }

    async fn update_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
        expected_config_version: u64,
        draft: ProviderEndpointDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_endpoint(
            expected,
            ProviderEndpointMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    async fn delete_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_endpoint(expected, ProviderEndpointMutation::Delete { id })
            .await
    }
}

impl SqliteStore {
    async fn mutate_proxy(
        &self,
        expected: ConfigRevision,
        mutation: ProxyMutation,
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
    mutation: ProxyMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }

    let Some(prepared) = prepare_mutation(current.proxies(), mutation)? else {
        return Ok((current, false));
    };
    execute_change(connection, prepared.change()).await?;
    let revision = bump_revision(connection, expected).await?;

    Ok((
        StoredConfiguration::new(
            revision,
            prepared.into_configuration(),
            current.provider_endpoints().clone(),
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

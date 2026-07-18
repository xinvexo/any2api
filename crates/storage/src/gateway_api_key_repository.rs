use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};
use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    gateway_api_key_mutation::{GatewayApiKeyMutation, prepare},
    gateway_api_key_writes::execute_change,
    proxy_repository::bump_revision,
    proxy_rows::load_configuration_from,
    sqlite::SqliteStore,
    vault::{SecretBytes, SecretVault},
};

#[async_trait]
pub trait GatewayApiKeyRepository: Send + Sync {
    async fn create_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
        token: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn update_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
        draft: GatewayApiKeyDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn rotate_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
        expected_token_version: u64,
        token: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn revoke_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError>;
}

#[async_trait]
impl GatewayApiKeyRepository for SqliteStore {
    async fn create_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
        token: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_timestamped(expected, |created_at| GatewayApiKeyMutation::Create {
            id,
            draft,
            token,
            created_at,
        })
        .await
    }

    async fn update_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
        draft: GatewayApiKeyDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_gateway_api_key(
            expected,
            GatewayApiKeyMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    async fn rotate_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
        expected_token_version: u64,
        token: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_gateway_api_key(
            expected,
            GatewayApiKeyMutation::Rotate {
                id,
                expected_config_version,
                expected_token_version,
                token,
            },
        )
        .await
    }

    async fn revoke_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_timestamped(expected, |revoked_at| GatewayApiKeyMutation::Revoke {
            id,
            expected_config_version,
            revoked_at,
        })
        .await
    }
}

impl SqliteStore {
    async fn mutate_gateway_api_key(
        &self,
        expected: ConfigRevision,
        mutation: GatewayApiKeyMutation,
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

    async fn mutate_timestamped<F>(
        &self,
        expected: ConfigRevision,
        build: F,
    ) -> Result<StoredConfiguration, StorageError>
    where
        F: FnOnce(String) -> GatewayApiKeyMutation + Send,
    {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let timestamp: String = sqlx::query_scalar("SELECT CURRENT_TIMESTAMP")
            .fetch_one(&mut *transaction)
            .await?;
        let (configuration, changed) = mutate_connection(
            &mut transaction,
            self.secret_vault(),
            expected,
            build(timestamp),
        )
        .await?;
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
    mutation: GatewayApiKeyMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) = prepare(
        current.gateway_api_keys(),
        current.gateway_api_key_verifier(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    execute_change(connection, prepared.change()).await?;
    let expected_keys = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    drop(current);
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.gateway_api_keys(), &expected_keys);
    Ok((configuration, true))
}

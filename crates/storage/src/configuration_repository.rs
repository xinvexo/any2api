use any2api_domain::{
    ConfigRevision, CredentialId, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId, ProxyDraft, ProxyProfileId,
};
use async_trait::async_trait;

use crate::{
    configuration::StoredConfiguration, error::StorageError,
    gateway_api_key_repository::GatewayApiKeyRepository,
    oauth_account_repository::OAuthAccountRepository,
    provider_credential_mutation::ProviderCredentialMutation,
    provider_endpoint_mutation::ProviderEndpointMutation, proxy_mutation::ProxyMutation,
    proxy_rows::load_configuration_from, settings_repository::SettingRepository,
    sqlite::SqliteStore, vault::SecretBytes,
};

#[async_trait]
pub trait ConfigurationRepository:
    GatewayApiKeyRepository + OAuthAccountRepository + SettingRepository + Send + Sync
{
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

    async fn set_proxy_authentication(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        username: String,
        password: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn clear_proxy_authentication(
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

    async fn create_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        api_key: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn update_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        draft: ProviderCredentialDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn rotate_provider_credential_secret(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        expected_secret_version: u64,
        api_key: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn set_provider_credential_models(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        models: Vec<String>,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn delete_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError>;
}

#[async_trait]
impl ConfigurationRepository for SqliteStore {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let configuration = load_configuration_from(&mut transaction, self.secret_vault()).await?;
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

    async fn set_proxy_authentication(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        username: String,
        password: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_proxy_authentication(
            expected,
            crate::proxy_auth_repository::ProxyAuthenticationMutation::Set {
                id,
                username,
                password,
            },
        )
        .await
    }

    async fn clear_proxy_authentication(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_proxy_authentication(
            expected,
            crate::proxy_auth_repository::ProxyAuthenticationMutation::Clear { id },
        )
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

    async fn create_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        api_key: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_credential(
            expected,
            ProviderCredentialMutation::Create {
                id,
                endpoint_id,
                draft,
                api_key,
            },
        )
        .await
    }

    async fn update_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        draft: ProviderCredentialDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_credential(
            expected,
            ProviderCredentialMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    async fn rotate_provider_credential_secret(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        expected_secret_version: u64,
        api_key: SecretBytes,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_credential(
            expected,
            ProviderCredentialMutation::RotateSecret {
                id,
                expected_config_version,
                expected_secret_version,
                api_key,
            },
        )
        .await
    }

    async fn set_provider_credential_models(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        models: Vec<String>,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_credential(
            expected,
            ProviderCredentialMutation::SetModels {
                id,
                expected_config_version,
                models,
            },
        )
        .await
    }

    async fn delete_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_provider_credential(
            expected,
            ProviderCredentialMutation::Delete {
                id,
                expected_config_version,
            },
        )
        .await
    }
}

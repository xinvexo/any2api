use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, ModelRouteDraft, ModelRouteId, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProxyDraft, ProxyProfileId,
};
use any2api_storage::api::ConfigurationRepository;

use crate::{
    config_command::{ConfigCommand, execute},
    config_publish_error::ConfigPublishError,
    provider_api_key_secret::ProviderApiKeySecret,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

pub struct ConfigPublisher {
    repository: Arc<dyn ConfigurationRepository>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
}

impl ConfigPublisher {
    #[must_use]
    pub fn new<R>(
        repository: Arc<R>,
        snapshots: Arc<SnapshotStore>,
        runtime: Arc<RuntimeRegistry>,
    ) -> Self
    where
        R: ConfigurationRepository + 'static,
    {
        Self {
            repository,
            snapshots,
            runtime,
        }
    }

    pub async fn create_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::CreateProxy { id, draft })
            .await
    }

    pub async fn update_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::UpdateProxy { id, draft })
            .await
    }

    pub async fn delete_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::DeleteProxy { id })
            .await
    }

    pub async fn set_global_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::SetGlobalProxy { id })
            .await
    }

    pub async fn create_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::CreateProviderEndpoint { id, draft },
        )
        .await
    }

    pub async fn update_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
        expected_config_version: u64,
        draft: ProviderEndpointDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::UpdateProviderEndpoint {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    pub async fn delete_provider_endpoint(
        &self,
        expected: ConfigRevision,
        id: ProviderEndpointId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::DeleteProviderEndpoint { id })
            .await
    }

    pub async fn create_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        api_key: ProviderApiKeySecret,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::CreateProviderCredential {
                id,
                endpoint_id,
                draft,
                api_key,
            },
        )
        .await
    }

    pub async fn update_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        draft: ProviderCredentialDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::UpdateProviderCredential {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    pub async fn rotate_provider_credential_secret(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        expected_secret_version: u64,
        api_key: ProviderApiKeySecret,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::RotateProviderCredentialSecret {
                id,
                expected_config_version,
                expected_secret_version,
                api_key,
            },
        )
        .await
    }

    pub async fn delete_provider_credential(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::DeleteProviderCredential {
                id,
                expected_config_version,
            },
        )
        .await
    }

    pub async fn create_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        draft: ModelRouteDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::CreateModelRoute { id, draft })
            .await
    }

    pub async fn update_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        expected_config_version: u64,
        draft: ModelRouteDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::UpdateModelRoute {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    pub async fn delete_model_route(
        &self,
        expected: ConfigRevision,
        id: ModelRouteId,
        expected_config_version: u64,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::DeleteModelRoute {
                id,
                expected_config_version,
            },
        )
        .await
    }

    async fn publish(
        &self,
        expected: ConfigRevision,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        if current.revision() != expected {
            return Err(ConfigPublishError::RevisionConflict {
                expected,
                actual: current.revision(),
            });
        }
        let committed = execute(self.repository.as_ref(), expected, command).await?;

        if committed.revision() == expected {
            return Ok(current);
        }
        let next = expected
            .checked_next()
            .expect("repository committed a revision after the persisted maximum");
        assert_eq!(
            committed.revision(),
            next,
            "repository committed an unexpected configuration revision"
        );
        let snapshot = PublishedSnapshot::new(committed, self.runtime.as_ref());
        let published = self.snapshots.replace(snapshot);
        self.runtime.advance_scheduler_epoch();
        Ok(published)
    }
}

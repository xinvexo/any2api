use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, ModelRouteDraft, ModelRouteId, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProxyDraft, ProxyProfileId, SettingKey,
    SettingValue,
};
use any2api_storage::api::ConfigurationRepository;

use crate::{
    config_command::{ConfigCommand, execute},
    config_publish_error::ConfigPublishError,
    logging_reconciler::LoggingSettingsReconciler,
    provider_api_key_secret::ProviderApiKeySecret,
    proxy_password_secret::ProxyPasswordSecret,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

#[derive(Clone)]
pub struct ConfigPublisher {
    pub(crate) repository: Arc<dyn ConfigurationRepository>,
    pub(crate) snapshots: Arc<SnapshotStore>,
    pub(crate) runtime: Arc<RuntimeRegistry>,
    logging_reconciler: Option<Arc<dyn LoggingSettingsReconciler>>,
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
            logging_reconciler: None,
        }
    }

    #[must_use]
    pub fn with_logging_reconciler(
        mut self,
        reconciler: Arc<dyn LoggingSettingsReconciler>,
    ) -> Self {
        self.logging_reconciler = Some(reconciler);
        self
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

    pub async fn set_proxy_authentication(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        username: String,
        password: ProxyPasswordSecret,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::SetProxyAuthentication {
                id,
                username,
                password,
            },
        )
        .await
    }

    pub async fn clear_proxy_authentication(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::ClearProxyAuthentication { id })
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

    pub async fn set_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::SetSettingOverride { key, value })
            .await
    }

    pub async fn reset_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::ResetSettingOverride { key })
            .await
    }

    async fn publish(
        &self,
        expected: ConfigRevision,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        crate::publish_task::run(self.runtime.lifecycle(), async move {
            publisher.publish_serialized(expected, command).await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn publish_serialized(
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
        Ok(self.publish_committed(current, expected, committed))
    }

    pub(crate) fn publish_committed(
        &self,
        current: Arc<PublishedSnapshot>,
        expected: ConfigRevision,
        committed: any2api_storage::api::StoredConfiguration,
    ) -> Arc<PublishedSnapshot> {
        if committed.revision() == expected {
            return current;
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
        if let Some(reconciler) = &self.logging_reconciler {
            reconciler.reconcile(snapshot.revision(), snapshot.settings().logging());
        }
        let published = self.snapshots.replace(snapshot);
        self.runtime.advance_scheduler_epoch();
        published
    }
}

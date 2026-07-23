use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId,
};

use super::ConfigPublisher;
use crate::{
    config_command::{ConfigCommand, execute},
    config_publish_error::ConfigPublishError,
    provider_api_key_secret::ProviderApiKeySecret,
    provider_oauth2_secret::ProviderOAuth2Secret,
    published_snapshot::PublishedSnapshot,
};

impl ConfigPublisher {
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

    pub(crate) async fn create_provider_oauth_credential(
        &self,
        id: CredentialId,
        endpoint_id: ProviderEndpointId,
        expected_endpoint_config_version: u64,
        draft: ProviderCredentialDraft,
        oauth_secret: ProviderOAuth2Secret,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        crate::publish_task::run(self.runtime.lifecycle(), async move {
            let _guard = publisher.snapshots.acquire_publish().await;
            let current = publisher.snapshots.load();
            let expected = current.revision();
            let command = ConfigCommand::CreateProviderOAuthCredential {
                id,
                endpoint_id,
                expected_endpoint_config_version,
                draft,
                oauth_secret,
            };
            publisher.validate_command(current.as_ref(), &command)?;
            let committed = execute(publisher.repository.as_ref(), expected, command).await?;
            Ok(publisher.publish_committed(current, expected, committed))
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
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

    pub(crate) async fn refresh_provider_oauth_credential_secret(
        &self,
        id: CredentialId,
        expected_secret_version: u64,
        oauth_secret: ProviderOAuth2Secret,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        crate::publish_task::run(self.runtime.lifecycle(), async move {
            let _guard = publisher.snapshots.acquire_publish().await;
            let current = publisher.snapshots.load();
            let expected = current.revision();
            let committed = execute(
                publisher.repository.as_ref(),
                expected,
                ConfigCommand::RefreshProviderOAuthCredentialSecret {
                    id,
                    expected_secret_version,
                    oauth_secret,
                },
            )
            .await?;
            Ok(publisher.publish_committed(current, expected, committed))
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
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

    pub async fn set_provider_credential_models(
        &self,
        expected: ConfigRevision,
        id: CredentialId,
        expected_config_version: u64,
        models: Vec<String>,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::SetProviderCredentialModels {
                id,
                expected_config_version,
                models,
            },
        )
        .await
    }

    pub(super) fn validate_command(
        &self,
        current: &PublishedSnapshot,
        command: &ConfigCommand,
    ) -> Result<(), ConfigPublishError> {
        match command {
            ConfigCommand::CreateProviderEndpoint { draft, .. }
            | ConfigCommand::UpdateProviderEndpoint { draft, .. } => {
                self.capabilities.validate_endpoint(
                    draft.provider_kind(),
                    draft.protocol_dialect(),
                    draft.effective_upstream_protocol_dialect(),
                )?
            }
            ConfigCommand::CreateProviderCredential {
                endpoint_id, draft, ..
            }
            | ConfigCommand::CreateProviderOAuthCredential {
                endpoint_id, draft, ..
            } => {
                let endpoint = current
                    .provider_endpoints()
                    .get(*endpoint_id)
                    .ok_or(ConfigPublishError::ProviderEndpointNotFound)?;
                self.capabilities
                    .validate_credential(endpoint.provider_kind(), draft.credential_kind())?;
            }
            ConfigCommand::UpdateProviderCredential { id, draft, .. } => {
                let credential = current
                    .provider_credentials()
                    .get(*id)
                    .ok_or(ConfigPublishError::ProviderCredentialNotFound)?;
                let endpoint = current
                    .provider_endpoints()
                    .get(credential.provider_endpoint_id())
                    .ok_or(ConfigPublishError::ProviderEndpointNotFound)?;
                self.capabilities
                    .validate_credential(endpoint.provider_kind(), draft.credential_kind())?;
            }
            _ => {}
        }
        Ok(())
    }
}

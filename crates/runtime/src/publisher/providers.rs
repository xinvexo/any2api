use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId,
};

use super::ConfigPublisher;
use crate::{
    config_command::ConfigCommand, config_publish_error::ConfigPublishError,
    provider_api_key_secret::ProviderApiKeySecret, published_snapshot::PublishedSnapshot,
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

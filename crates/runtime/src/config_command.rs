use any2api_domain::{
    ConfigRevision, CredentialId, ModelRouteDraft, ModelRouteId, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProxyDraft, ProxyProfileId, SettingKey,
    SettingValue,
};
use any2api_storage::api::{ConfigurationRepository, StoredConfiguration};

use crate::{
    config_publish_error::ConfigPublishError, provider_api_key_secret::ProviderApiKeySecret,
};

pub(crate) enum ConfigCommand {
    CreateProxy {
        id: ProxyProfileId,
        draft: ProxyDraft,
    },
    UpdateProxy {
        id: ProxyProfileId,
        draft: ProxyDraft,
    },
    DeleteProxy {
        id: ProxyProfileId,
    },
    SetGlobalProxy {
        id: ProxyProfileId,
    },
    CreateProviderEndpoint {
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
    },
    UpdateProviderEndpoint {
        id: ProviderEndpointId,
        expected_config_version: u64,
        draft: ProviderEndpointDraft,
    },
    DeleteProviderEndpoint {
        id: ProviderEndpointId,
    },
    CreateProviderCredential {
        id: CredentialId,
        endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        api_key: ProviderApiKeySecret,
    },
    UpdateProviderCredential {
        id: CredentialId,
        expected_config_version: u64,
        draft: ProviderCredentialDraft,
    },
    RotateProviderCredentialSecret {
        id: CredentialId,
        expected_config_version: u64,
        expected_secret_version: u64,
        api_key: ProviderApiKeySecret,
    },
    DeleteProviderCredential {
        id: CredentialId,
        expected_config_version: u64,
    },
    CreateModelRoute {
        id: ModelRouteId,
        draft: ModelRouteDraft,
    },
    UpdateModelRoute {
        id: ModelRouteId,
        expected_config_version: u64,
        draft: ModelRouteDraft,
    },
    DeleteModelRoute {
        id: ModelRouteId,
        expected_config_version: u64,
    },
    SetSettingOverride {
        key: SettingKey,
        value: SettingValue,
    },
    ResetSettingOverride {
        key: SettingKey,
    },
}

pub(crate) async fn execute(
    repository: &dyn ConfigurationRepository,
    expected: ConfigRevision,
    command: ConfigCommand,
) -> Result<StoredConfiguration, ConfigPublishError> {
    let result = match command {
        ConfigCommand::CreateProxy { id, draft } => {
            repository.create_proxy(expected, id, draft).await
        }
        ConfigCommand::UpdateProxy { id, draft } => {
            repository.update_proxy(expected, id, draft).await
        }
        ConfigCommand::DeleteProxy { id } => repository.delete_proxy(expected, id).await,
        ConfigCommand::SetGlobalProxy { id } => repository.set_global_proxy(expected, id).await,
        ConfigCommand::CreateProviderEndpoint { id, draft } => {
            repository
                .create_provider_endpoint(expected, id, draft)
                .await
        }
        ConfigCommand::UpdateProviderEndpoint {
            id,
            expected_config_version,
            draft,
        } => {
            repository
                .update_provider_endpoint(expected, id, expected_config_version, draft)
                .await
        }
        ConfigCommand::DeleteProviderEndpoint { id } => {
            repository.delete_provider_endpoint(expected, id).await
        }
        ConfigCommand::CreateProviderCredential {
            id,
            endpoint_id,
            draft,
            api_key,
        } => {
            repository
                .create_provider_credential(
                    expected,
                    id,
                    endpoint_id,
                    draft,
                    api_key.into_storage_secret(),
                )
                .await
        }
        ConfigCommand::UpdateProviderCredential {
            id,
            expected_config_version,
            draft,
        } => {
            repository
                .update_provider_credential(expected, id, expected_config_version, draft)
                .await
        }
        ConfigCommand::RotateProviderCredentialSecret {
            id,
            expected_config_version,
            expected_secret_version,
            api_key,
        } => {
            repository
                .rotate_provider_credential_secret(
                    expected,
                    id,
                    expected_config_version,
                    expected_secret_version,
                    api_key.into_storage_secret(),
                )
                .await
        }
        ConfigCommand::DeleteProviderCredential {
            id,
            expected_config_version,
        } => {
            repository
                .delete_provider_credential(expected, id, expected_config_version)
                .await
        }
        ConfigCommand::CreateModelRoute { id, draft } => {
            repository.create_model_route(expected, id, draft).await
        }
        ConfigCommand::UpdateModelRoute {
            id,
            expected_config_version,
            draft,
        } => {
            repository
                .update_model_route(expected, id, expected_config_version, draft)
                .await
        }
        ConfigCommand::DeleteModelRoute {
            id,
            expected_config_version,
        } => {
            repository
                .delete_model_route(expected, id, expected_config_version)
                .await
        }
        ConfigCommand::SetSettingOverride { key, value } => {
            repository.set_setting_override(expected, key, value).await
        }
        ConfigCommand::ResetSettingOverride { key } => {
            repository.reset_setting_override(expected, key).await
        }
    };
    result.map_err(ConfigPublishError::from)
}

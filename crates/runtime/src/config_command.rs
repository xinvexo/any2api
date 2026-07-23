use any2api_domain::{
    ConfigRevision, CredentialId, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId, ProxyDraft, ProxyProfileId, SettingKey, SettingValue,
};
use any2api_storage::api::{ConfigurationRepository, StoredConfiguration};

use crate::{
    config_publish_error::ConfigPublishError, provider_api_key_secret::ProviderApiKeySecret,
    provider_oauth2_secret::ProviderOAuth2Secret, proxy_password_secret::ProxyPasswordSecret,
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
    SetProxyAuthentication {
        id: ProxyProfileId,
        username: String,
        password: ProxyPasswordSecret,
    },
    ClearProxyAuthentication {
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
    CreateProviderOAuthCredential {
        id: CredentialId,
        endpoint_id: ProviderEndpointId,
        expected_endpoint_config_version: u64,
        draft: ProviderCredentialDraft,
        oauth_secret: ProviderOAuth2Secret,
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
    RefreshProviderOAuthCredentialSecret {
        id: CredentialId,
        expected_secret_version: u64,
        oauth_secret: ProviderOAuth2Secret,
    },
    SetProviderCredentialModels {
        id: CredentialId,
        expected_config_version: u64,
        models: Vec<String>,
    },
    DeleteProviderCredential {
        id: CredentialId,
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
        ConfigCommand::SetProxyAuthentication {
            id,
            username,
            password,
        } => {
            repository
                .set_proxy_authentication(expected, id, username, password.into_storage_secret())
                .await
        }
        ConfigCommand::ClearProxyAuthentication { id } => {
            repository.clear_proxy_authentication(expected, id).await
        }
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
        ConfigCommand::CreateProviderOAuthCredential {
            id,
            endpoint_id,
            expected_endpoint_config_version,
            draft,
            oauth_secret,
        } => {
            repository
                .create_provider_oauth_credential(
                    expected,
                    id,
                    endpoint_id,
                    expected_endpoint_config_version,
                    draft,
                    oauth_secret.into_storage_secret(),
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
        ConfigCommand::RefreshProviderOAuthCredentialSecret {
            id,
            expected_secret_version,
            oauth_secret,
        } => {
            repository
                .refresh_provider_oauth_credential_secret(
                    expected,
                    id,
                    expected_secret_version,
                    oauth_secret.into_storage_secret(),
                )
                .await
        }
        ConfigCommand::SetProviderCredentialModels {
            id,
            expected_config_version,
            models,
        } => {
            repository
                .set_provider_credential_models(expected, id, expected_config_version, models)
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
        ConfigCommand::SetSettingOverride { key, value } => {
            repository.set_setting_override(expected, key, value).await
        }
        ConfigCommand::ResetSettingOverride { key } => {
            repository.reset_setting_override(expected, key).await
        }
    };
    result.map_err(ConfigPublishError::from)
}

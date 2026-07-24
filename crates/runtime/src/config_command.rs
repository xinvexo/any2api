use any2api_domain::{
    ConfigRevision, CredentialId, OAuthAccountDraft, OAuthAccountId, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyDraft, ProxyProfileId,
    SettingKey, SettingValue,
};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument, StoredConfiguration};

use crate::{
    config_publish_error::ConfigPublishError, provider_api_key_secret::ProviderApiKeySecret,
    proxy_password_secret::ProxyPasswordSecret,
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
    SetProviderCredentialModels {
        id: CredentialId,
        expected_config_version: u64,
        models: Vec<String>,
    },
    DeleteProviderCredential {
        id: CredentialId,
        expected_config_version: u64,
    },
    CreateOAuthAccount {
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    },
    UpdateOAuthAccount {
        id: OAuthAccountId,
        expected_config_version: u64,
        draft: OAuthAccountDraft,
    },
    SetOAuthAccountModels {
        id: OAuthAccountId,
        expected_config_version: u64,
        models: Vec<String>,
    },
    RefreshOAuthAccount {
        id: OAuthAccountId,
        expected_token_version: u64,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        document: OAuthAccountDocument,
    },
    DeleteOAuthAccount {
        id: OAuthAccountId,
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
        ConfigCommand::CreateOAuthAccount {
            id,
            provider_kind,
            draft,
            safe_account_email,
            expires_at,
            models,
            document,
        } => {
            repository
                .create_oauth_account(
                    expected,
                    id,
                    provider_kind,
                    draft,
                    safe_account_email,
                    expires_at,
                    models,
                    document,
                )
                .await
        }
        ConfigCommand::UpdateOAuthAccount {
            id,
            expected_config_version,
            draft,
        } => {
            repository
                .update_oauth_account(expected, id, expected_config_version, draft)
                .await
        }
        ConfigCommand::SetOAuthAccountModels {
            id,
            expected_config_version,
            models,
        } => {
            repository
                .set_oauth_account_models(expected, id, expected_config_version, models)
                .await
        }
        ConfigCommand::RefreshOAuthAccount {
            id,
            expected_token_version,
            safe_account_email,
            expires_at,
            document,
        } => {
            repository
                .refresh_oauth_account(
                    expected,
                    id,
                    expected_token_version,
                    safe_account_email,
                    expires_at,
                    document,
                )
                .await
        }
        ConfigCommand::DeleteOAuthAccount {
            id,
            expected_config_version,
        } => {
            repository
                .delete_oauth_account(expected, id, expected_config_version)
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

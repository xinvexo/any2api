use any2api_domain::SettingOverrideChange;
use any2api_storage::api::ConfigurationMutation;

use super::ConfigCommand;

impl ConfigCommand {
    pub(crate) fn into_mutation(self) -> ConfigurationMutation {
        match self {
            Self::CreateProxy { id, draft } => ConfigurationMutation::CreateProxy { id, draft },
            Self::UpdateProxy { id, draft } => ConfigurationMutation::UpdateProxy { id, draft },
            Self::DeleteProxy { id } => ConfigurationMutation::DeleteProxy { id },
            Self::SetGlobalProxy { id } => ConfigurationMutation::SetGlobalProxy { id },
            Self::SetProxyAuthentication {
                id,
                username,
                password,
            } => ConfigurationMutation::SetProxyAuthentication {
                id,
                username,
                password: password.into_storage_secret(),
            },
            Self::ClearProxyAuthentication { id } => {
                ConfigurationMutation::ClearProxyAuthentication { id }
            }
            Self::CreateProviderEndpoint { id, draft } => {
                ConfigurationMutation::CreateProviderEndpoint { id, draft }
            }
            Self::UpdateProviderEndpoint {
                id,
                expected_config_version,
                draft,
            } => ConfigurationMutation::UpdateProviderEndpoint {
                id,
                expected_config_version,
                draft,
            },
            Self::DeleteProviderEndpoint { id } => {
                ConfigurationMutation::DeleteProviderEndpoint { id }
            }
            Self::CreateProviderCredential {
                id,
                endpoint_id,
                draft,
                api_key,
            } => ConfigurationMutation::CreateProviderCredential {
                id,
                endpoint_id,
                draft,
                api_key: api_key.into_storage_secret(),
            },
            Self::UpdateProviderCredential {
                id,
                expected_config_version,
                draft,
            } => ConfigurationMutation::UpdateProviderCredential {
                id,
                expected_config_version,
                draft,
            },
            Self::RotateProviderCredentialSecret {
                id,
                expected_config_version,
                expected_secret_version,
                api_key,
            } => ConfigurationMutation::RotateProviderCredentialSecret {
                id,
                expected_config_version,
                expected_secret_version,
                api_key: api_key.into_storage_secret(),
            },
            Self::SetProviderCredentialModels {
                id,
                expected_config_version,
                models,
            } => ConfigurationMutation::SetProviderCredentialModels {
                id,
                expected_config_version,
                models,
            },
            Self::DeleteProviderCredential {
                id,
                expected_config_version,
            } => ConfigurationMutation::DeleteProviderCredential {
                id,
                expected_config_version,
            },
            Self::CreateOAuthAccount {
                id,
                provider_kind,
                draft,
                safe_account_email,
                expires_at,
                models,
                document,
            } => ConfigurationMutation::CreateOAuthAccount {
                id,
                provider_kind,
                draft,
                safe_account_email,
                expires_at,
                models,
                document,
            },
            Self::UpdateOAuthAccount {
                id,
                expected_config_version,
                draft,
            } => ConfigurationMutation::UpdateOAuthAccount {
                id,
                expected_config_version,
                draft,
            },
            Self::SetOAuthAccountModels {
                id,
                expected_config_version,
                models,
            } => ConfigurationMutation::SetOAuthAccountModels {
                id,
                expected_config_version,
                models,
            },
            Self::RefreshOAuthAccount {
                id,
                expected_token_version,
                safe_account_email,
                expires_at,
                document,
            } => ConfigurationMutation::RefreshOAuthAccount {
                id,
                expected_token_version,
                safe_account_email,
                expires_at,
                document,
            },
            Self::DeleteOAuthAccount {
                id,
                expected_config_version,
            } => ConfigurationMutation::DeleteOAuthAccount {
                id,
                expected_config_version,
            },
            Self::CreateGatewayApiKey { id, draft, token } => {
                ConfigurationMutation::CreateGatewayApiKey { id, draft, token }
            }
            Self::UpdateGatewayApiKey {
                id,
                expected_config_version,
                draft,
            } => ConfigurationMutation::UpdateGatewayApiKey {
                id,
                expected_config_version,
                draft,
            },
            Self::RotateGatewayApiKey {
                id,
                expected_config_version,
                expected_token_version,
                token,
            } => ConfigurationMutation::RotateGatewayApiKey {
                id,
                expected_config_version,
                expected_token_version,
                token,
            },
            Self::DeleteGatewayApiKey {
                id,
                expected_config_version,
            } => ConfigurationMutation::DeleteGatewayApiKey {
                id,
                expected_config_version,
            },
            Self::SetSettingOverride { key, value } => ConfigurationMutation::ApplySettingChanges {
                changes: vec![SettingOverrideChange::Set { key, value }],
            },
            Self::ResetSettingOverride { key } => ConfigurationMutation::ApplySettingChanges {
                changes: vec![SettingOverrideChange::Reset { key }],
            },
            Self::ApplySettingChanges { changes } => {
                ConfigurationMutation::ApplySettingChanges { changes }
            }
        }
    }
}

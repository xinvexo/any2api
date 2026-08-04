use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    error::StorageError,
    gateway_api_key::{GatewayApiKeyMutation, mutate_gateway_api_key_configuration},
    oauth_account::{
        OAuthAccountMutation, mutate_oauth_account_configuration,
        mutate_oauth_account_create_batch, mutate_oauth_account_refresh_batch,
    },
    provider::{
        ProviderCredentialMutation, ProviderEndpointMutation,
        mutate_provider_credential_configuration, mutate_provider_endpoint_configuration,
    },
    proxy::{
        ProxyAuthenticationMutation, ProxyMutation, mutate_proxy_authentication_configuration,
        mutate_proxy_configuration,
    },
    settings::mutate_settings_configuration,
};

use super::super::{ConfigurationMutation, StoredConfiguration};

pub(super) async fn execute_mutation(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: ConfigurationMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    macro_rules! execute {
        ($function:ident, $mutation:expr $(,)?) => {
            $function(connection, expected, $mutation).await
        };
    }

    match mutation {
        ConfigurationMutation::CreateProxy { id, draft } => execute!(
            mutate_proxy_configuration,
            ProxyMutation::Create { id, draft },
        ),
        ConfigurationMutation::UpdateProxy { id, draft } => execute!(
            mutate_proxy_configuration,
            ProxyMutation::Update { id, draft },
        ),
        ConfigurationMutation::DeleteProxy { id } => {
            execute!(mutate_proxy_configuration, ProxyMutation::Delete { id },)
        }
        ConfigurationMutation::SetGlobalProxy { id } => {
            execute!(mutate_proxy_configuration, ProxyMutation::SetGlobal { id },)
        }
        ConfigurationMutation::SetProxyAuthentication {
            id,
            username,
            password,
        } => execute!(
            mutate_proxy_authentication_configuration,
            ProxyAuthenticationMutation::Set {
                id,
                username,
                password,
            },
        ),
        ConfigurationMutation::ClearProxyAuthentication { id } => execute!(
            mutate_proxy_authentication_configuration,
            ProxyAuthenticationMutation::Clear { id },
        ),
        ConfigurationMutation::CreateProviderEndpoint { id, draft } => execute!(
            mutate_provider_endpoint_configuration,
            ProviderEndpointMutation::Create { id, draft },
        ),
        ConfigurationMutation::UpdateProviderEndpoint {
            id,
            expected_config_version,
            draft,
        } => execute!(
            mutate_provider_endpoint_configuration,
            ProviderEndpointMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        ),
        ConfigurationMutation::DeleteProviderEndpoint { id } => execute!(
            mutate_provider_endpoint_configuration,
            ProviderEndpointMutation::Delete { id },
        ),
        ConfigurationMutation::CreateProviderCredential {
            id,
            endpoint_id,
            draft,
            api_key,
        } => execute!(
            mutate_provider_credential_configuration,
            ProviderCredentialMutation::Create {
                id,
                endpoint_id,
                draft,
                api_key,
            },
        ),
        ConfigurationMutation::UpdateProviderCredential {
            id,
            expected_config_version,
            draft,
        } => execute!(
            mutate_provider_credential_configuration,
            ProviderCredentialMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        ),
        ConfigurationMutation::RotateProviderCredentialSecret {
            id,
            expected_config_version,
            expected_secret_version,
            api_key,
        } => execute!(
            mutate_provider_credential_configuration,
            ProviderCredentialMutation::RotateSecret {
                id,
                expected_config_version,
                expected_secret_version,
                api_key,
            },
        ),
        ConfigurationMutation::SetProviderCredentialModels {
            id,
            expected_config_version,
            models,
        } => execute!(
            mutate_provider_credential_configuration,
            ProviderCredentialMutation::SetModels {
                id,
                expected_config_version,
                models,
            },
        ),
        ConfigurationMutation::DeleteProviderCredential {
            id,
            expected_config_version,
        } => execute!(
            mutate_provider_credential_configuration,
            ProviderCredentialMutation::Delete {
                id,
                expected_config_version,
            },
        ),
        ConfigurationMutation::CreateOAuthAccount {
            id,
            provider_kind,
            draft,
            safe_account_email,
            expires_at,
            models,
            document,
        } => {
            let created_at = current_timestamp(connection).await?;
            execute!(
                mutate_oauth_account_configuration,
                OAuthAccountMutation::Create {
                    id,
                    provider_kind,
                    draft,
                    safe_account_email,
                    expires_at,
                    created_at,
                    models,
                    document,
                },
            )
        }
        ConfigurationMutation::CreateOAuthAccounts { accounts } => {
            mutate_oauth_account_create_batch(connection, expected, accounts).await
        }
        ConfigurationMutation::ReauthorizeOAuthAccount {
            id,
            expected_token_version,
            safe_account_email,
            expires_at,
            models,
            document,
        } => execute!(
            mutate_oauth_account_configuration,
            OAuthAccountMutation::Reauthorize {
                id,
                expected_token_version,
                safe_account_email,
                expires_at,
                models,
                document,
            },
        ),
        ConfigurationMutation::UpdateOAuthAccount {
            id,
            expected_config_version,
            draft,
        } => execute!(
            mutate_oauth_account_configuration,
            OAuthAccountMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        ),
        ConfigurationMutation::SetOAuthAccountModels {
            id,
            expected_config_version,
            models,
        } => execute!(
            mutate_oauth_account_configuration,
            OAuthAccountMutation::SetModels {
                id,
                expected_config_version,
                models,
            },
        ),
        ConfigurationMutation::RefreshOAuthAccount {
            id,
            expected_token_version,
            safe_account_email,
            expires_at,
            document,
        } => execute!(
            mutate_oauth_account_configuration,
            OAuthAccountMutation::Refresh {
                id,
                expected_token_version,
                safe_account_email,
                expires_at,
                document,
            },
        ),
        ConfigurationMutation::RefreshOAuthAccounts { refreshes } => {
            mutate_oauth_account_refresh_batch(connection, expected, refreshes).await
        }
        ConfigurationMutation::DeleteOAuthAccount {
            id,
            expected_config_version,
        } => execute!(
            mutate_oauth_account_configuration,
            OAuthAccountMutation::Delete {
                id,
                expected_config_version,
            },
        ),
        ConfigurationMutation::CreateGatewayApiKey { id, draft, token } => {
            let created_at = current_timestamp(connection).await?;
            execute!(
                mutate_gateway_api_key_configuration,
                GatewayApiKeyMutation::Create {
                    id,
                    draft,
                    token,
                    created_at,
                },
            )
        }
        ConfigurationMutation::UpdateGatewayApiKey {
            id,
            expected_config_version,
            draft,
        } => execute!(
            mutate_gateway_api_key_configuration,
            GatewayApiKeyMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        ),
        ConfigurationMutation::RotateGatewayApiKey {
            id,
            expected_config_version,
            expected_token_version,
            token,
        } => execute!(
            mutate_gateway_api_key_configuration,
            GatewayApiKeyMutation::Rotate {
                id,
                expected_config_version,
                expected_token_version,
                token,
            },
        ),
        ConfigurationMutation::DeleteGatewayApiKey {
            id,
            expected_config_version,
        } => execute!(
            mutate_gateway_api_key_configuration,
            GatewayApiKeyMutation::Delete {
                id,
                expected_config_version,
            },
        ),
        ConfigurationMutation::ApplySettingChanges { changes } => {
            mutate_settings_configuration(connection, expected, changes).await
        }
    }
}

async fn current_timestamp(connection: &mut SqliteConnection) -> Result<String, StorageError> {
    sqlx::query_scalar("SELECT CURRENT_TIMESTAMP")
        .fetch_one(connection)
        .await
        .map_err(Into::into)
}

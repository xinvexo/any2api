mod mutation;

use any2api_domain::{
    CredentialId, GatewayApiKeyDraft, GatewayApiKeyId, OAuthAccountDraft, OAuthAccountId,
    OAuthProxySelection, ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId,
    ProviderKind, ProxyDraft, ProxyProfileId, SettingKey, SettingOverrideChange, SettingValue,
};
use any2api_storage::api::{OAuthAccountDocument, SecretBytes};

use crate::{credential::ProviderApiKeySecret, proxy::ProxyPasswordSecret};

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
        proxy_selection: OAuthProxySelection,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    },
    UpdateOAuthAccount {
        id: OAuthAccountId,
        expected_config_version: u64,
        draft: OAuthAccountDraft,
        proxy_selection: OAuthProxySelection,
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
    CreateGatewayApiKey {
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
        token: SecretBytes,
    },
    UpdateGatewayApiKey {
        id: GatewayApiKeyId,
        expected_config_version: u64,
        draft: GatewayApiKeyDraft,
    },
    RotateGatewayApiKey {
        id: GatewayApiKeyId,
        expected_config_version: u64,
        expected_token_version: u64,
        token: SecretBytes,
    },
    DeleteGatewayApiKey {
        id: GatewayApiKeyId,
        expected_config_version: u64,
    },
    SetSettingOverride {
        key: SettingKey,
        value: SettingValue,
    },
    ResetSettingOverride {
        key: SettingKey,
    },
    ApplySettingChanges {
        changes: Vec<SettingOverrideChange>,
    },
}

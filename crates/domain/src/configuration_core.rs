use crate::{
    ConfigRevision, GatewayApiKeyConfiguration, ModelRouteConfiguration, OAuthAccountConfiguration,
    ProviderCredentialConfiguration, ProviderEndpointConfiguration, ProxyConfiguration,
    SettingsConfiguration,
};

/// The persisted configuration shared unchanged by storage, candidate
/// compilation and the published runtime snapshot.
#[derive(Debug)]
pub struct ConfigurationCore {
    revision: ConfigRevision,
    proxies: ProxyConfiguration,
    provider_endpoints: ProviderEndpointConfiguration,
    provider_credentials: ProviderCredentialConfiguration,
    oauth_accounts: OAuthAccountConfiguration,
    model_routes: ModelRouteConfiguration,
    gateway_api_keys: GatewayApiKeyConfiguration,
    settings: SettingsConfiguration,
}

impl ConfigurationCore {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        revision: ConfigRevision,
        proxies: ProxyConfiguration,
        provider_endpoints: ProviderEndpointConfiguration,
        provider_credentials: ProviderCredentialConfiguration,
        oauth_accounts: OAuthAccountConfiguration,
        model_routes: ModelRouteConfiguration,
        gateway_api_keys: GatewayApiKeyConfiguration,
        settings: SettingsConfiguration,
    ) -> Self {
        Self {
            revision,
            proxies,
            provider_endpoints,
            provider_credentials,
            oauth_accounts,
            model_routes,
            gateway_api_keys,
            settings,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    #[must_use]
    pub const fn proxies(&self) -> &ProxyConfiguration {
        &self.proxies
    }

    #[must_use]
    pub const fn provider_endpoints(&self) -> &ProviderEndpointConfiguration {
        &self.provider_endpoints
    }

    #[must_use]
    pub const fn provider_credentials(&self) -> &ProviderCredentialConfiguration {
        &self.provider_credentials
    }

    #[must_use]
    pub const fn oauth_accounts(&self) -> &OAuthAccountConfiguration {
        &self.oauth_accounts
    }

    #[must_use]
    pub const fn model_routes(&self) -> &ModelRouteConfiguration {
        &self.model_routes
    }

    #[must_use]
    pub const fn gateway_api_keys(&self) -> &GatewayApiKeyConfiguration {
        &self.gateway_api_keys
    }

    #[must_use]
    pub const fn settings(&self) -> &SettingsConfiguration {
        &self.settings
    }

    #[must_use]
    pub fn into_parts(self) -> ConfigurationCoreParts {
        ConfigurationCoreParts {
            revision: self.revision,
            proxies: self.proxies,
            provider_endpoints: self.provider_endpoints,
            provider_credentials: self.provider_credentials,
            oauth_accounts: self.oauth_accounts,
            model_routes: self.model_routes,
            gateway_api_keys: self.gateway_api_keys,
            settings: self.settings,
        }
    }

    #[must_use]
    pub fn from_parts(parts: ConfigurationCoreParts) -> Self {
        Self {
            revision: parts.revision,
            proxies: parts.proxies,
            provider_endpoints: parts.provider_endpoints,
            provider_credentials: parts.provider_credentials,
            oauth_accounts: parts.oauth_accounts,
            model_routes: parts.model_routes,
            gateway_api_keys: parts.gateway_api_keys,
            settings: parts.settings,
        }
    }
}

pub struct ConfigurationCoreParts {
    pub revision: ConfigRevision,
    pub proxies: ProxyConfiguration,
    pub provider_endpoints: ProviderEndpointConfiguration,
    pub provider_credentials: ProviderCredentialConfiguration,
    pub oauth_accounts: OAuthAccountConfiguration,
    pub model_routes: ModelRouteConfiguration,
    pub gateway_api_keys: GatewayApiKeyConfiguration,
    pub settings: SettingsConfiguration,
}

use any2api_domain::{
    ConfigRevision, GatewayApiKeyConfiguration, ModelRouteConfiguration,
    ProviderCredentialConfiguration, ProviderEndpointConfiguration, ProxyConfiguration,
};

use crate::{
    gateway_api_key_verifier::GatewayApiKeyVerifier,
    provider_credential_secret_material::StoredProviderCredentialSecrets,
};

#[derive(Debug)]
pub struct StoredConfiguration {
    revision: ConfigRevision,
    proxies: ProxyConfiguration,
    provider_endpoints: ProviderEndpointConfiguration,
    provider_credentials: ProviderCredentialConfiguration,
    model_routes: ModelRouteConfiguration,
    gateway_api_keys: GatewayApiKeyConfiguration,
    gateway_api_key_verifier: GatewayApiKeyVerifier,
    provider_credential_secrets: StoredProviderCredentialSecrets,
}

impl StoredConfiguration {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        revision: ConfigRevision,
        proxies: ProxyConfiguration,
        provider_endpoints: ProviderEndpointConfiguration,
        provider_credentials: ProviderCredentialConfiguration,
        model_routes: ModelRouteConfiguration,
        gateway_api_keys: GatewayApiKeyConfiguration,
        gateway_api_key_verifier: GatewayApiKeyVerifier,
        provider_credential_secrets: StoredProviderCredentialSecrets,
    ) -> Self {
        Self {
            revision,
            proxies,
            provider_endpoints,
            provider_credentials,
            model_routes,
            gateway_api_keys,
            gateway_api_key_verifier,
            provider_credential_secrets,
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
    pub const fn model_routes(&self) -> &ModelRouteConfiguration {
        &self.model_routes
    }

    #[must_use]
    pub const fn gateway_api_keys(&self) -> &GatewayApiKeyConfiguration {
        &self.gateway_api_keys
    }

    pub(crate) const fn gateway_api_key_verifier(&self) -> &GatewayApiKeyVerifier {
        &self.gateway_api_key_verifier
    }

    #[cfg(test)]
    pub(crate) const fn provider_credential_secrets(&self) -> &StoredProviderCredentialSecrets {
        &self.provider_credential_secrets
    }

    #[must_use]
    pub fn into_parts(self) -> StoredConfigurationParts {
        StoredConfigurationParts {
            revision: self.revision,
            proxies: self.proxies,
            provider_endpoints: self.provider_endpoints,
            provider_credentials: self.provider_credentials,
            model_routes: self.model_routes,
            gateway_api_keys: self.gateway_api_keys,
            gateway_api_key_verifier: self.gateway_api_key_verifier,
            provider_credential_secrets: self.provider_credential_secrets,
        }
    }
}

pub struct StoredConfigurationParts {
    pub revision: ConfigRevision,
    pub proxies: ProxyConfiguration,
    pub provider_endpoints: ProviderEndpointConfiguration,
    pub provider_credentials: ProviderCredentialConfiguration,
    pub model_routes: ModelRouteConfiguration,
    pub gateway_api_keys: GatewayApiKeyConfiguration,
    pub gateway_api_key_verifier: GatewayApiKeyVerifier,
    pub provider_credential_secrets: StoredProviderCredentialSecrets,
}

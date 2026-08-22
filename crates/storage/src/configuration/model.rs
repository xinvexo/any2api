use any2api_domain::{
    ConfigRevision, ConfigurationCore, ConfigurationCoreParts, GatewayApiKeyConfiguration,
    ModelRouteConfiguration, OAuthAccountConfiguration, ProviderCredentialConfiguration,
    ProviderEndpointConfiguration, ProxyConfiguration, SettingsConfiguration,
};

use crate::{
    oauth_account::StoredOAuthAccountMaterials, provider::StoredProviderCredentialSecrets,
    proxy::StoredProxyPasswords,
};

#[derive(Debug)]
pub struct StoredConfiguration {
    core: ConfigurationCore,
    provider_credential_secrets: StoredProviderCredentialSecrets,
    oauth_account_materials: StoredOAuthAccountMaterials,
    proxy_passwords: StoredProxyPasswords,
}

impl StoredConfiguration {
    #[must_use]
    pub const fn new(
        core: ConfigurationCore,
        provider_credential_secrets: StoredProviderCredentialSecrets,
        oauth_account_materials: StoredOAuthAccountMaterials,
        proxy_passwords: StoredProxyPasswords,
    ) -> Self {
        Self {
            core,
            provider_credential_secrets,
            oauth_account_materials,
            proxy_passwords,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.core.revision()
    }

    #[must_use]
    pub const fn proxies(&self) -> &ProxyConfiguration {
        self.core.proxies()
    }

    #[must_use]
    pub const fn provider_endpoints(&self) -> &ProviderEndpointConfiguration {
        self.core.provider_endpoints()
    }

    #[must_use]
    pub const fn provider_credentials(&self) -> &ProviderCredentialConfiguration {
        self.core.provider_credentials()
    }

    #[must_use]
    pub const fn oauth_accounts(&self) -> &OAuthAccountConfiguration {
        self.core.oauth_accounts()
    }

    #[must_use]
    pub const fn model_routes(&self) -> &ModelRouteConfiguration {
        self.core.model_routes()
    }

    #[must_use]
    pub const fn gateway_api_keys(&self) -> &GatewayApiKeyConfiguration {
        self.core.gateway_api_keys()
    }

    #[must_use]
    pub const fn settings(&self) -> &SettingsConfiguration {
        self.core.settings()
    }

    #[cfg(test)]
    pub(crate) const fn provider_credential_secrets(&self) -> &StoredProviderCredentialSecrets {
        &self.provider_credential_secrets
    }

    #[cfg(test)]
    pub(crate) const fn oauth_account_materials(&self) -> &StoredOAuthAccountMaterials {
        &self.oauth_account_materials
    }

    #[cfg(test)]
    pub(crate) const fn proxy_passwords(&self) -> &StoredProxyPasswords {
        &self.proxy_passwords
    }

    #[must_use]
    pub fn into_parts(self) -> StoredConfigurationParts {
        StoredConfigurationParts {
            core: self.core.into_parts(),
            provider_credential_secrets: self.provider_credential_secrets,
            oauth_account_materials: self.oauth_account_materials,
            proxy_passwords: self.proxy_passwords,
        }
    }

    pub(crate) fn from_parts(parts: StoredConfigurationParts) -> Self {
        Self {
            core: ConfigurationCore::from_parts(parts.core),
            provider_credential_secrets: parts.provider_credential_secrets,
            oauth_account_materials: parts.oauth_account_materials,
            proxy_passwords: parts.proxy_passwords,
        }
    }
}

pub struct StoredConfigurationParts {
    pub core: ConfigurationCoreParts,
    pub provider_credential_secrets: StoredProviderCredentialSecrets,
    pub oauth_account_materials: StoredOAuthAccountMaterials,
    pub proxy_passwords: StoredProxyPasswords,
}

use std::collections::{HashMap, HashSet};

use crate::{
    CredentialId, ProviderCredential, ProviderCredentialValidationError,
    ProviderEndpointConfiguration, ProviderEndpointId, ProxyConfiguration, ProxyProfileId,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderCredentialConfiguration {
    credentials: Vec<ProviderCredential>,
}

impl ProviderCredentialConfiguration {
    pub fn new(
        mut credentials: Vec<ProviderCredential>,
        endpoints: &ProviderEndpointConfiguration,
        proxies: &ProxyConfiguration,
    ) -> Result<Self, ProviderCredentialValidationError> {
        let mut ids = HashSet::new();
        let mut labels = HashMap::new();
        for credential in &credentials {
            if !ids.insert(credential.id()) {
                return Err(ProviderCredentialValidationError::DuplicateId);
            }
            if labels
                .insert(
                    (credential.provider_endpoint_id(), credential.label_key()),
                    credential.id(),
                )
                .is_some()
            {
                return Err(ProviderCredentialValidationError::DuplicateLabel);
            }
            if endpoints.get(credential.provider_endpoint_id()).is_none() {
                return Err(ProviderCredentialValidationError::MissingProviderEndpoint);
            }
            if proxies.get(credential.proxy_profile_id()).is_none() {
                return Err(ProviderCredentialValidationError::MissingProxyProfile);
            }
        }
        credentials.sort_by(|left, right| {
            left.provider_endpoint_id()
                .cmp(&right.provider_endpoint_id())
                .then_with(|| left.label().cmp(right.label()))
        });
        Ok(Self { credentials })
    }

    #[must_use]
    pub const fn initial() -> Self {
        Self {
            credentials: Vec::new(),
        }
    }

    #[must_use]
    pub fn credentials(&self) -> &[ProviderCredential] {
        &self.credentials
    }

    #[must_use]
    pub fn get(&self, id: CredentialId) -> Option<&ProviderCredential> {
        self.credentials
            .iter()
            .find(|credential| credential.id() == id)
    }

    pub fn for_endpoint(
        &self,
        endpoint_id: ProviderEndpointId,
    ) -> impl Iterator<Item = &ProviderCredential> {
        self.credentials
            .iter()
            .filter(move |credential| credential.provider_endpoint_id() == endpoint_id)
    }

    #[must_use]
    pub fn references_endpoint(&self, endpoint_id: ProviderEndpointId) -> bool {
        self.for_endpoint(endpoint_id).next().is_some()
    }

    #[must_use]
    pub fn references_proxy(&self, proxy_id: ProxyProfileId) -> bool {
        self.credentials
            .iter()
            .any(|credential| credential.proxy_profile_id() == proxy_id)
    }

    pub fn with_endpoint_generation_incremented(
        &self,
        endpoint_id: ProviderEndpointId,
        endpoints: &ProviderEndpointConfiguration,
        proxies: &ProxyConfiguration,
    ) -> Result<Self, ProviderCredentialValidationError> {
        let credentials = self
            .credentials
            .iter()
            .map(|credential| {
                if credential.provider_endpoint_id() == endpoint_id {
                    credential.next_generation()
                } else {
                    Ok(credential.clone())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(credentials, endpoints, proxies)
    }
}

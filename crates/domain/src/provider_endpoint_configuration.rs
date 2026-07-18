use std::collections::{HashMap, HashSet};

use crate::{ProviderEndpoint, ProviderEndpointId, ProviderEndpointValidationError};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderEndpointConfiguration {
    endpoints: Vec<ProviderEndpoint>,
}

impl ProviderEndpointConfiguration {
    pub fn new(
        mut endpoints: Vec<ProviderEndpoint>,
    ) -> Result<Self, ProviderEndpointValidationError> {
        let mut ids = HashSet::new();
        let mut names = HashMap::new();
        for endpoint in &endpoints {
            if !ids.insert(endpoint.id()) {
                return Err(ProviderEndpointValidationError::DuplicateId);
            }
            if names.insert(endpoint.name_key(), endpoint.id()).is_some() {
                return Err(ProviderEndpointValidationError::DuplicateName);
            }
        }
        endpoints.sort_by(|left, right| {
            left.provider_kind()
                .cmp(&right.provider_kind())
                .then_with(|| left.name().cmp(right.name()))
        });
        Ok(Self { endpoints })
    }

    #[must_use]
    pub const fn initial() -> Self {
        Self {
            endpoints: Vec::new(),
        }
    }

    #[must_use]
    pub fn endpoints(&self) -> &[ProviderEndpoint] {
        &self.endpoints
    }

    #[must_use]
    pub fn get(&self, id: ProviderEndpointId) -> Option<&ProviderEndpoint> {
        self.endpoints.iter().find(|endpoint| endpoint.id() == id)
    }
}

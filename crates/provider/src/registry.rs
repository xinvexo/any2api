use std::{collections::HashMap, sync::Arc};

use any2api_domain::ProviderKind;

use crate::{ProviderError, api::ProviderDriver};

#[derive(Default)]
pub struct ProviderRegistry {
    drivers: HashMap<ProviderKind, Arc<dyn ProviderDriver>>,
}

impl ProviderRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, driver: Arc<dyn ProviderDriver>) -> Result<(), ProviderError> {
        let kind = driver.kind();
        if self.drivers.contains_key(&kind) {
            return Err(ProviderError::DuplicateProvider(kind));
        }

        self.drivers.insert(kind, driver);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, kind: ProviderKind) -> Option<&Arc<dyn ProviderDriver>> {
        self.drivers.get(&kind)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProviderKind, &Arc<dyn ProviderDriver>)> {
        self.drivers.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use any2api_domain::{
        ProtocolOperation, ProviderBaseUrl, ProviderKind, RetrySafety, UpstreamErrorClassification,
        UpstreamErrorKind,
    };
    use http::HeaderMap;

    use super::ProviderRegistry;
    use crate::{
        ProviderError, ProviderSecret,
        api::{
            CapabilitySet, CredentialHeaders, EndpointPlan, ProviderDriver, UpstreamResponseMeta,
        },
    };

    struct FakeDriver {
        capabilities: CapabilitySet,
    }

    impl FakeDriver {
        fn new() -> Self {
            Self {
                capabilities: CapabilitySet::default(),
            }
        }
    }

    impl ProviderDriver for FakeDriver {
        fn kind(&self) -> ProviderKind {
            ProviderKind::Codex
        }

        fn capabilities(&self) -> &CapabilitySet {
            &self.capabilities
        }

        fn validate_credential(&self, _secret: &ProviderSecret) -> Result<(), ProviderError> {
            Ok(())
        }

        fn endpoint_plan(
            &self,
            base_url: &ProviderBaseUrl,
            _operation: ProtocolOperation,
        ) -> Result<EndpointPlan, ProviderError> {
            Ok(EndpointPlan {
                url: url::Url::parse(base_url.as_str()).expect("validated URL"),
            })
        }

        fn credential_headers(
            &self,
            _secret: &ProviderSecret,
        ) -> Result<CredentialHeaders, ProviderError> {
            Ok(CredentialHeaders {
                headers: HeaderMap::new(),
            })
        }

        fn classify_error(
            &self,
            _operation: ProtocolOperation,
            _meta: &UpstreamResponseMeta,
            _bounded_body: &[u8],
        ) -> UpstreamErrorClassification {
            UpstreamErrorClassification::new(
                UpstreamErrorKind::Unknown,
                RetrySafety::Ambiguous,
                None,
            )
        }
    }

    #[test]
    fn duplicate_provider_kinds_are_rejected() {
        let mut registry = ProviderRegistry::new();
        registry
            .register(Arc::new(FakeDriver::new()))
            .expect("first driver registers");

        let error = registry
            .register(Arc::new(FakeDriver::new()))
            .expect_err("duplicate driver must fail");

        assert_eq!(error, ProviderError::DuplicateProvider(ProviderKind::Codex));
    }
}

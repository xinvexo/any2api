use std::{collections::HashMap, sync::Arc};

use any2api_domain::ProviderKind;

use crate::{
    ProviderError,
    api::{OAuthLoginFlow, ProviderDriver},
};

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
        validate_driver(driver.as_ref())?;
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

fn validate_driver(driver: &dyn ProviderDriver) -> Result<(), ProviderError> {
    let descriptor = *driver.descriptor();
    let invalid = |reason| ProviderError::InvalidProviderDescriptor {
        provider: descriptor.kind(),
        reason,
    };
    descriptor.validate().map_err(invalid)?;

    let Some(oauth) = descriptor.oauth() else {
        if driver.oauth_authorization_code().is_some()
            || driver.oauth_device_code().is_some()
            || driver.oauth_token().is_some()
            || driver.oauth_routing().is_some()
            || driver.oauth_quota().is_some()
        {
            return Err(invalid("OAuth facet is present without OAuth capabilities"));
        }
        return Ok(());
    };

    if driver.oauth_token().is_none() || driver.oauth_routing().is_none() {
        return Err(invalid("OAuth token or routing facet is missing"));
    }
    let login_matches = match oauth.login_flow() {
        OAuthLoginFlow::AuthorizationCodePkce => {
            driver.oauth_authorization_code().is_some() && driver.oauth_device_code().is_none()
        }
        OAuthLoginFlow::DeviceCode => {
            driver.oauth_device_code().is_some() && driver.oauth_authorization_code().is_none()
        }
    };
    if !login_matches {
        return Err(invalid("OAuth login facet does not match the descriptor"));
    }

    let quota = driver.oauth_quota();
    if oauth.supports_quota() != quota.is_some() {
        return Err(invalid("OAuth quota facet does not match the descriptor"));
    }
    if let Some(quota) = quota {
        if oauth.supports_quota_supplement() != quota.supplement().is_some() {
            return Err(invalid(
                "OAuth quota supplement facet does not match the descriptor",
            ));
        }
        if oauth.supports_quota_reset() != quota.reset().is_some() {
            return Err(invalid(
                "OAuth quota reset facet does not match the descriptor",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use any2api_domain::{
        ProtocolOperation, ProviderBaseUrl, ProviderKind, RetrySafety, TransportMode,
        UpstreamError, UpstreamErrorClassification, UpstreamErrorKind,
    };
    use http::HeaderMap;

    use super::ProviderRegistry;
    use crate::{
        ProviderError, ProviderSecret,
        api::{
            CredentialHeaders, CredentialTestPlan, EndpointPlan, OAuthCapabilities, OAuthLoginFlow,
            ProviderDescriptor, ProviderDriver, UpstreamResponseMeta,
        },
    };

    struct FakeDriver {
        descriptor: &'static ProviderDescriptor,
    }

    impl FakeDriver {
        fn new(descriptor: &'static ProviderDescriptor) -> Self {
            Self { descriptor }
        }
    }

    const API_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Codex,
        &[ProtocolOperation::Responses],
        None,
        &[TransportMode::Json],
    );

    const INCONSISTENT_OAUTH_DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
        ProviderKind::Claude,
        &[],
        Some(OAuthCapabilities::new(
            &[ProtocolOperation::Messages],
            OAuthLoginFlow::AuthorizationCodePkce,
        )),
        &[TransportMode::Json],
    );

    impl ProviderDriver for FakeDriver {
        fn descriptor(&self) -> &'static ProviderDescriptor {
            self.descriptor
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
            _base_url: &ProviderBaseUrl,
            _secret: &ProviderSecret,
        ) -> Result<CredentialHeaders, ProviderError> {
            Ok(CredentialHeaders {
                headers: HeaderMap::new(),
            })
        }

        fn credential_test_plan(
            &self,
            base_url: &ProviderBaseUrl,
        ) -> Result<CredentialTestPlan, ProviderError> {
            Ok(CredentialTestPlan {
                url: url::Url::parse(base_url.as_str()).expect("validated URL"),
                headers: HeaderMap::new(),
            })
        }

        fn parse_model_catalog(&self, _bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
            Ok(Vec::new())
        }

        fn classify_error(
            &self,
            _operation: ProtocolOperation,
            _meta: &UpstreamResponseMeta,
            _bounded_body: &[u8],
        ) -> UpstreamError {
            UpstreamError::new(
                UpstreamErrorClassification::new(
                    UpstreamErrorKind::Unknown,
                    RetrySafety::Ambiguous,
                    None,
                ),
                None,
            )
        }
    }

    #[test]
    fn duplicate_provider_kinds_are_rejected() {
        let mut registry = ProviderRegistry::new();
        registry
            .register(Arc::new(FakeDriver::new(&API_DESCRIPTOR)))
            .expect("first driver registers");

        let error = registry
            .register(Arc::new(FakeDriver::new(&API_DESCRIPTOR)))
            .expect_err("duplicate driver must fail");

        assert_eq!(error, ProviderError::DuplicateProvider(ProviderKind::Codex));
    }

    #[test]
    fn descriptor_and_optional_facets_must_agree() {
        let mut registry = ProviderRegistry::new();
        let error = registry
            .register(Arc::new(FakeDriver::new(&INCONSISTENT_OAUTH_DESCRIPTOR)))
            .expect_err("missing OAuth facets must fail registration");

        assert_eq!(
            error,
            ProviderError::InvalidProviderDescriptor {
                provider: ProviderKind::Claude,
                reason: "OAuth token or routing facet is missing",
            }
        );
    }
}

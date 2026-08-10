use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind,
    TransportMode,
};
use http::HeaderMap;

use super::{headers as kimi_headers, upstream_error as kimi_error};
use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, CredentialTestPlan, EndpointPlan, ProviderDriver,
        UpstreamResponseMeta,
    },
    credential::api_key,
};

#[derive(Debug)]
pub struct KimiDriver {
    capabilities: CapabilitySet,
}

impl Default for KimiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl KimiDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: CapabilitySet {
                protocols: [ProtocolDialect::OpenAiChatCompletions]
                    .into_iter()
                    .collect(),
                transport_modes: [TransportMode::Json, TransportMode::Sse]
                    .into_iter()
                    .collect(),
                credential_kinds: [CredentialKind::ApiKey].into_iter().collect(),
            },
        }
    }
}

impl ProviderDriver for KimiDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Kimi
    }

    fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError> {
        api_key::validate_secret(secret)
    }

    fn endpoint_plan(
        &self,
        base_url: &ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        if operation != ProtocolOperation::ChatCompletions {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Kimi".into(),
            ));
        }
        Ok(EndpointPlan {
            url: api_key::endpoint_url(base_url, operation)?,
        })
    }

    fn credential_test_plan(
        &self,
        base_url: &ProviderBaseUrl,
    ) -> Result<CredentialTestPlan, ProviderError> {
        Ok(CredentialTestPlan {
            url: api_key::credential_test_url(base_url)?,
            headers: HeaderMap::new(),
        })
    }

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        api_key::parse_model_catalog(bounded_body)
    }

    fn credential_headers(
        &self,
        _base_url: &ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        api_key::bearer_credential_headers(secret)
    }

    fn response_headers(&self, _operation: ProtocolOperation, upstream: &HeaderMap) -> HeaderMap {
        kimi_headers::response(upstream)
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamError {
        kimi_error::classify(meta, bounded_body)
    }
}

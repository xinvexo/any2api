use any2api_domain::{
    CredentialKind, OpenAiChatCompletionsProfile, ProtocolDialect, ProtocolOperation,
    ProtocolTargetProfile, ProviderBaseUrl, ProviderKind, TransportMode,
};
use http::HeaderMap;

use super::headers as openai_headers;
use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, CredentialTestPlan, EndpointPlan, ProviderDriver,
        UpstreamResponseMeta,
    },
    credential::api_key,
    upstream_error::openai as openai_error,
};

#[derive(Debug)]
pub struct OpenAiDriver {
    capabilities: CapabilitySet,
}

impl Default for OpenAiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAiDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: CapabilitySet {
                protocols: [
                    ProtocolDialect::OpenAiResponses,
                    ProtocolDialect::OpenAiChatCompletions,
                    ProtocolDialect::OpenAiImages,
                ]
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

impl ProviderDriver for OpenAiDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAi
    }

    fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    fn supports_api_key_operation(&self, operation: ProtocolOperation) -> bool {
        matches!(
            operation,
            ProtocolOperation::Responses
                | ProtocolOperation::ResponsesCompact
                | ProtocolOperation::ChatCompletions
                | ProtocolOperation::ImagesGenerations
                | ProtocolOperation::ImagesEdits
        )
    }

    fn protocol_target_profile(
        &self,
        dialect: ProtocolDialect,
        _upstream_model: &str,
    ) -> Option<ProtocolTargetProfile> {
        (dialect == ProtocolDialect::OpenAiChatCompletions).then_some(
            ProtocolTargetProfile::OpenAiChatCompletions(
                OpenAiChatCompletionsProfile::CURRENT_OPENAI,
            ),
        )
    }

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError> {
        api_key::validate_secret(secret)
    }

    fn endpoint_plan(
        &self,
        base_url: &ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        if !self.supports_api_key_operation(operation) {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by OpenAI".into(),
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
        openai_headers::response(upstream)
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamError {
        openai_error::classify(meta, bounded_body)
    }
}

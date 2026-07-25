use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};

use crate::{
    ProviderError, ProviderSecret,
    api::{CapabilitySet, CredentialHeaders, EndpointPlan, ProviderDriver, UpstreamResponseMeta},
    api_key, openai_error,
};

#[derive(Debug)]
pub struct GrokDriver {
    capabilities: CapabilitySet,
}

impl Default for GrokDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: CapabilitySet {
                protocols: [
                    ProtocolDialect::OpenAiResponses,
                    ProtocolDialect::OpenAiChatCompletions,
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

impl ProviderDriver for GrokDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Grok
    }

    fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError> {
        api_key::validate_secret(secret)
    }

    fn endpoint_plan(
        &self,
        base_url: &any2api_domain::ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        if !matches!(
            operation,
            ProtocolOperation::Responses
                | ProtocolOperation::ResponsesCompact
                | ProtocolOperation::ChatCompletions
        ) {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Grok".into(),
            ));
        }
        Ok(EndpointPlan {
            url: api_key::endpoint_url(base_url, operation)?,
        })
    }

    fn credential_test_plan(
        &self,
        base_url: &any2api_domain::ProviderBaseUrl,
    ) -> Result<EndpointPlan, ProviderError> {
        Ok(EndpointPlan {
            url: api_key::credential_test_url(base_url)?,
        })
    }

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        api_key::parse_model_catalog(bounded_body)
    }

    fn credential_headers(
        &self,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        api_key::bearer_credential_headers(secret)
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamErrorClassification {
        openai_error::classify(meta, bounded_body)
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{
        ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, TransportMode,
    };
    use http::header::AUTHORIZATION;

    use super::GrokDriver;
    use crate::{ProviderSecret, api::ProviderDriver};

    #[test]
    fn builds_xai_paths_and_bearer_authentication() {
        let driver = GrokDriver::new();
        let base = ProviderBaseUrl::parse("https://api.x.ai/v1").expect("base URL");

        assert_eq!(driver.kind(), ProviderKind::Grok);
        assert_eq!(
            driver
                .endpoint_plan(&base, ProtocolOperation::ResponsesCompact)
                .expect("compact endpoint")
                .url
                .as_str(),
            "https://api.x.ai/v1/responses/compact"
        );
        assert_eq!(
            driver
                .endpoint_plan(&base, ProtocolOperation::ChatCompletions)
                .expect("chat endpoint")
                .url
                .as_str(),
            "https://api.x.ai/v1/chat/completions"
        );
        assert_eq!(
            driver
                .credential_test_plan(&base)
                .expect("models endpoint")
                .url
                .as_str(),
            "https://api.x.ai/v1/models"
        );
        let headers = driver
            .credential_headers(&ProviderSecret::new(1, "xai-test-key"))
            .expect("headers");
        assert_eq!(headers.headers[AUTHORIZATION], "Bearer xai-test-key");
        assert!(!format!("{headers:?}").contains("xai-test-key"));
        assert!(
            driver
                .capabilities()
                .protocols
                .contains(&ProtocolDialect::OpenAiResponses)
        );
        assert!(
            driver
                .capabilities()
                .transport_modes
                .contains(&TransportMode::Sse)
        );
        assert_eq!(driver.oauth_redirect_uri(), None);
    }

    #[test]
    fn rejects_anthropic_operations() {
        let driver = GrokDriver::new();
        let base = ProviderBaseUrl::parse("https://api.x.ai/v1").expect("base URL");

        assert!(
            driver
                .endpoint_plan(&base, ProtocolOperation::Messages)
                .is_err()
        );
    }
}

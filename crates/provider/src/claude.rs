use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};
use http::{HeaderMap, HeaderValue};

use crate::{
    ProviderError, ProviderSecret,
    api::{CapabilitySet, CredentialHeaders, EndpointPlan, ProviderDriver, UpstreamResponseMeta},
    api_key, claude_error,
};

#[derive(Debug)]
pub struct ClaudeDriver {
    capabilities: CapabilitySet,
}

impl Default for ClaudeDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: CapabilitySet {
                protocols: [ProtocolDialect::AnthropicMessages].into_iter().collect(),
                transport_modes: [TransportMode::Json, TransportMode::Sse]
                    .into_iter()
                    .collect(),
                credential_kinds: [CredentialKind::ApiKey].into_iter().collect(),
            },
        }
    }
}

impl ProviderDriver for ClaudeDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Claude
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
            ProtocolOperation::Messages | ProtocolOperation::MessagesCountTokens
        ) {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Claude".into(),
            ));
        }
        Ok(EndpointPlan {
            url: api_key::endpoint_url(base_url, operation)?,
        })
    }

    fn credential_headers(
        &self,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        self.validate_credential(secret)?;
        let api_key = HeaderValue::from_str(secret.expose())
            .map_err(|_| ProviderError::InvalidCredential("invalid API Key header".into()))?;
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", api_key);
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        Ok(CredentialHeaders { headers })
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

    fn classify_error(
        &self,
        operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamErrorClassification {
        claude_error::classify(operation, meta, bounded_body)
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolOperation, ProviderBaseUrl, TransportMode, UpstreamErrorKind};
    use http::{HeaderMap, StatusCode};

    use super::ClaudeDriver;
    use crate::{
        ProviderSecret,
        api::{ProviderDriver, UpstreamResponseMeta},
    };

    #[test]
    fn builds_messages_paths_and_anthropic_headers() {
        let driver = ClaudeDriver::new();
        let base = ProviderBaseUrl::parse("https://api.example.com/v1").expect("base URL");
        assert_eq!(
            driver
                .endpoint_plan(&base, ProtocolOperation::MessagesCountTokens)
                .expect("endpoint")
                .url
                .as_str(),
            "https://api.example.com/v1/messages/count_tokens"
        );
        assert_eq!(
            driver
                .credential_test_plan(&base)
                .expect("credential test endpoint")
                .url
                .as_str(),
            "https://api.example.com/v1/models"
        );
        let headers = driver
            .credential_headers(&ProviderSecret::new(1, "sk-claude"))
            .expect("headers");
        assert_eq!(headers.headers["x-api-key"], "sk-claude");
        assert_eq!(headers.headers["anthropic-version"], "2023-06-01");
        assert!(
            driver
                .capabilities()
                .transport_modes
                .contains(&TransportMode::Sse)
        );
    }

    #[test]
    fn count_tokens_not_found_is_operation_unavailable() {
        let driver = ClaudeDriver::new();
        let response = UpstreamResponseMeta {
            status: StatusCode::NOT_FOUND,
            headers: HeaderMap::new(),
        };

        assert_eq!(
            driver
                .classify_error(ProtocolOperation::MessagesCountTokens, &response, b"{}")
                .kind(),
            UpstreamErrorKind::OperationUnavailable
        );
        assert_eq!(
            driver
                .classify_error(ProtocolOperation::Messages, &response, b"{}")
                .kind(),
            UpstreamErrorKind::ModelUnavailable
        );
    }
}

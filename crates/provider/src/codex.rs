use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};
use http::{HeaderMap, HeaderValue, header};

use crate::{
    ProviderError, ProviderSecret,
    api::{CapabilitySet, CredentialHeaders, EndpointPlan, ProviderDriver, UpstreamResponseMeta},
    api_key, codex_error,
};

#[derive(Debug)]
pub struct CodexDriver {
    capabilities: CapabilitySet,
}

impl Default for CodexDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: CapabilitySet {
                protocols: [ProtocolDialect::OpenAiResponses].into_iter().collect(),
                transport_modes: [TransportMode::Json, TransportMode::Sse]
                    .into_iter()
                    .collect(),
                credential_kinds: [CredentialKind::ApiKey].into_iter().collect(),
            },
        }
    }
}

impl ProviderDriver for CodexDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Codex
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
            ProtocolOperation::Responses | ProtocolOperation::ResponsesCompact
        ) {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Codex".into(),
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
        let value = format!("Bearer {}", secret.expose());
        let authorization = HeaderValue::from_str(&value)
            .map_err(|_| ProviderError::InvalidCredential("invalid API Key header".into()))?;
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, authorization);
        Ok(CredentialHeaders { headers })
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamErrorClassification {
        codex_error::classify(meta, bounded_body)
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolOperation, ProviderBaseUrl, TransportMode};
    use http::header::AUTHORIZATION;

    use super::CodexDriver;
    use crate::{ProviderSecret, api::ProviderDriver};

    #[test]
    fn builds_responses_paths_and_bearer_authentication() {
        let driver = CodexDriver::new();
        let base =
            ProviderBaseUrl::parse("https://api.example.com/v1", false, false).expect("base URL");
        assert_eq!(
            driver
                .endpoint_plan(&base, ProtocolOperation::ResponsesCompact)
                .expect("endpoint")
                .url
                .as_str(),
            "https://api.example.com/v1/responses/compact"
        );
        let headers = driver
            .credential_headers(&ProviderSecret::new(1, "sk-codex"))
            .expect("headers");
        assert_eq!(headers.headers[AUTHORIZATION], "Bearer sk-codex");
        assert!(!format!("{headers:?}").contains("sk-codex"));
        assert!(
            driver
                .capabilities()
                .transport_modes
                .contains(&TransportMode::Sse)
        );
    }
}

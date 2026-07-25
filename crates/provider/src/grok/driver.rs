use super::oauth as grok_oauth;
use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, EndpointPlan, OAuthDeviceAuthorization,
        OAuthDeviceTokenPoll, OAuthGrant, OAuthLoginFlow, OAuthRequestPlan, OAuthRoutingProfile,
        OAuthTokenMaterial, ProviderDriver, UpstreamResponseMeta,
    },
    api_key, openai_error,
};
use http::HeaderMap;

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

    fn oauth_login_flow(&self) -> Option<OAuthLoginFlow> {
        Some(OAuthLoginFlow::DeviceCode)
    }

    fn oauth_device_authorization_request(&self) -> Result<OAuthRequestPlan, ProviderError> {
        grok_oauth::device_authorization_request()
    }

    fn parse_oauth_device_authorization(
        &self,
        body: &[u8],
    ) -> Result<OAuthDeviceAuthorization, ProviderError> {
        grok_oauth::parse_device_authorization(body)
    }

    fn oauth_device_token_request(
        &self,
        device_code: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        grok_oauth::device_token_request(device_code)
    }

    fn parse_oauth_device_token(
        &self,
        status: http::StatusCode,
        body: &[u8],
    ) -> Result<OAuthDeviceTokenPoll, ProviderError> {
        grok_oauth::parse_device_token(status, body)
    }

    fn oauth_token_request(
        &self,
        grant: OAuthGrant,
        code: &str,
        _state: Option<&str>,
        _code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        if grant != OAuthGrant::RefreshToken {
            return Err(ProviderError::InvalidCredential(
                "Grok uses OAuth device authorization".into(),
            ));
        }
        grok_oauth::refresh_request(code)
    }

    fn parse_oauth_token(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        grok_oauth::parse_token(body)
    }

    fn oauth_routing_profile(
        &self,
        _token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        grok_oauth::routing_profile()
    }

    fn oauth_supports_operation(&self, operation: ProtocolOperation) -> bool {
        operation == ProtocolOperation::Responses
    }

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        _forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        grok_oauth::credential_headers(token)
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

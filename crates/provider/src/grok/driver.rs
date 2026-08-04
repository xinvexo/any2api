use super::{
    headers as grok_headers, import as grok_import, oauth as grok_oauth, quota as grok_quota,
    upstream_error as grok_error,
};
use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, CredentialTestPlan, EndpointPlan,
        OAuthDeviceAuthorization, OAuthDeviceTokenPoll, OAuthGrant, OAuthImportedAccount,
        OAuthLoginFlow, OAuthQuotaQueryPlan, OAuthQuotaRejection, OAuthQuotaUsage,
        OAuthRequestPlan, OAuthRoutingProfile, OAuthTokenMaterial, ProviderDriver,
        ProviderRequestHeaderContext, UpstreamResponseMeta,
    },
    credential::api_key,
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
        _base_url: &any2api_domain::ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        api_key::bearer_credential_headers(secret)
    }

    fn prepare_request_headers(
        &self,
        context: ProviderRequestHeaderContext<'_>,
    ) -> Result<HeaderMap, ProviderError> {
        grok_headers::request(context)
    }

    fn response_headers(&self, _operation: ProtocolOperation, upstream: &HeaderMap) -> HeaderMap {
        grok_headers::response(upstream)
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

    fn parse_oauth_import(
        &self,
        object: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<OAuthImportedAccount>, ProviderError> {
        grok_import::parse(object)
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

    fn oauth_quota_query_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<Option<OAuthQuotaQueryPlan>, ProviderError> {
        grok_quota::query_plan(token).map(Some)
    }

    fn classify_oauth_quota_rejection(
        &self,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> OAuthQuotaRejection {
        grok_error::classify_oauth_quota_rejection(meta, bounded_body)
    }

    fn parse_oauth_quota_usage(
        &self,
        _meta: &UpstreamResponseMeta,
        body: &[u8],
    ) -> Result<OAuthQuotaUsage, ProviderError> {
        grok_quota::parse_usage(body)
    }

    fn parse_oauth_quota_supplement(
        &self,
        body: &[u8],
    ) -> Result<crate::api::OAuthQuotaSupplement, ProviderError> {
        grok_quota::parse_subscription(body)
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamError {
        grok_error::classify(meta, bounded_body)
    }
}

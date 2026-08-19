use super::{
    error as claude_error, headers as claude_headers, import as claude_import,
    model_catalog as claude_model_catalog, oauth as claude_oauth, quota as claude_quota,
};
use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind,
    RequestSpeedTier, TransportMode,
};
use http::{HeaderMap, HeaderValue};
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, CredentialTestPlan, EndpointPlan, OAuthGrant,
        OAuthImportedAccount, OAuthLoginFlow, OAuthModelCatalogScope, OAuthQuotaQueryPlan,
        OAuthQuotaUsage, OAuthRequestPlan, OAuthRoutingProfile, OAuthTokenMaterial, ProviderDriver,
        ProviderRequestContext, UpstreamResponseMeta,
    },
    credential::api_key,
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
        base_url: &ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        let path = match operation {
            ProtocolOperation::Messages => "v1/messages",
            ProtocolOperation::MessagesCountTokens => "v1/messages/count_tokens",
            _ => {
                return Err(ProviderError::InvalidEndpoint(
                    "operation is not supported by Claude".into(),
                ));
            }
        };
        Ok(EndpointPlan {
            url: api_key::path_url(base_url, path)?,
        })
    }

    fn credential_headers(
        &self,
        base_url: &ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        if base_url.as_str() != "https://api.anthropic.com" {
            return api_key::bearer_credential_headers(secret);
        }
        self.validate_credential(secret)?;
        let mut api_key = HeaderValue::from_str(secret.expose())
            .map_err(|_| ProviderError::InvalidCredential("invalid API Key header".into()))?;
        api_key.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", api_key);
        Ok(CredentialHeaders { headers })
    }

    fn prepare_request_headers(
        &self,
        context: ProviderRequestContext<'_>,
    ) -> Result<HeaderMap, ProviderError> {
        claude_headers::request(context)
    }

    fn request_speed_tier(
        &self,
        context: ProviderRequestContext<'_>,
        requested_speed_tier: Option<RequestSpeedTier>,
    ) -> Option<RequestSpeedTier> {
        (context.upstream_operation == ProtocolOperation::Messages)
            .then_some(requested_speed_tier)
            .flatten()
    }

    fn response_speed_tier(
        &self,
        operation: ProtocolOperation,
        observed_speed_tier: Option<RequestSpeedTier>,
    ) -> Option<RequestSpeedTier> {
        (operation == ProtocolOperation::Messages)
            .then_some(observed_speed_tier)
            .flatten()
    }

    fn response_headers(&self, _operation: ProtocolOperation, upstream: &HeaderMap) -> HeaderMap {
        claude_headers::response(upstream)
    }

    fn oauth_login_flow(&self) -> Option<OAuthLoginFlow> {
        Some(OAuthLoginFlow::AuthorizationCodePkce)
    }

    fn credential_test_plan(
        &self,
        base_url: &ProviderBaseUrl,
    ) -> Result<CredentialTestPlan, ProviderError> {
        let mut headers = HeaderMap::new();
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        Ok(CredentialTestPlan {
            url: api_key::path_url(base_url, "v1/models")?,
            headers,
        })
    }

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        api_key::parse_model_catalog(bounded_body)
    }

    fn oauth_redirect_uri(&self) -> Option<&'static str> {
        Some(claude_oauth::redirect_uri())
    }

    fn oauth_authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
    ) -> Result<Url, ProviderError> {
        claude_oauth::authorization_url(state, code_challenge)
    }

    fn oauth_token_request(
        &self,
        grant: OAuthGrant,
        code: &str,
        state: Option<&str>,
        code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        claude_oauth::token_request(grant, code, state, code_verifier)
    }

    fn parse_oauth_token_response(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        claude_oauth::parse_token(body)
    }

    fn parse_oauth_import(
        &self,
        object: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<OAuthImportedAccount>, ProviderError> {
        claude_import::parse(object)
    }

    fn oauth_routing_profile(
        &self,
        _token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        claude_oauth::routing_profile()
    }

    fn oauth_model_catalog_scope(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthModelCatalogScope, ProviderError> {
        claude_model_catalog::scope(token)
    }

    fn oauth_model_catalog_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        claude_model_catalog::request_plan(token)
    }

    fn parse_oauth_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        claude_model_catalog::parse(bounded_body)
    }

    fn oauth_supports_operation(&self, operation: ProtocolOperation) -> bool {
        matches!(
            operation,
            ProtocolOperation::Messages | ProtocolOperation::MessagesCountTokens
        )
    }

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        claude_oauth::credential_headers(token, forwarded)
    }

    fn oauth_quota_query_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<Option<OAuthQuotaQueryPlan>, ProviderError> {
        claude_quota::query_plan(token).map(Some)
    }

    fn parse_oauth_quota_usage(
        &self,
        _meta: &UpstreamResponseMeta,
        body: &[u8],
    ) -> Result<OAuthQuotaUsage, ProviderError> {
        claude_quota::parse_usage(body)
    }

    fn classify_error(
        &self,
        operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamError {
        claude_error::classify(operation, meta, bounded_body)
    }
}

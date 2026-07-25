use super::{import as codex_import, oauth as codex_oauth, quota as codex_quota};
use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};
use http::HeaderMap;
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, EndpointPlan, OAuthGrant, OAuthImportedAccount,
        OAuthLoginFlow, OAuthQuotaUsageParse, OAuthRequestPlan, OAuthRoutingProfile,
        OAuthTokenMaterial, ProviderDriver, UpstreamResponseMeta,
    },
    credential::api_key,
    upstream_error::openai as openai_error,
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
            ProtocolOperation::Responses
                | ProtocolOperation::ResponsesCompact
                | ProtocolOperation::ChatCompletions
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
        api_key::bearer_credential_headers(secret)
    }

    fn oauth_login_flow(&self) -> Option<OAuthLoginFlow> {
        Some(OAuthLoginFlow::AuthorizationCodePkce)
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

    fn oauth_redirect_uri(&self) -> Option<&'static str> {
        Some(codex_oauth::redirect_uri())
    }

    fn oauth_authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
    ) -> Result<Url, ProviderError> {
        codex_oauth::authorization_url(state, code_challenge)
    }

    fn oauth_token_request(
        &self,
        grant: OAuthGrant,
        code: &str,
        _state: Option<&str>,
        code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        codex_oauth::token_request(grant, code, code_verifier)
    }

    fn parse_oauth_token(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        codex_oauth::parse_token(body)
    }

    fn parse_oauth_import(
        &self,
        object: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<OAuthImportedAccount>, ProviderError> {
        codex_import::parse(object)
    }

    fn oauth_routing_profile(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        codex_oauth::routing_profile(token)
    }

    fn oauth_supports_operation(&self, operation: ProtocolOperation) -> bool {
        matches!(
            operation,
            ProtocolOperation::Responses | ProtocolOperation::ResponsesCompact
        )
    }

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        _forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        codex_oauth::credential_headers(token)
    }

    fn oauth_quota_query_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<Option<crate::api::OAuthQuotaQueryPlan>, ProviderError> {
        codex_quota::query_plan(token).map(Some)
    }

    fn parse_oauth_quota_usage(
        &self,
        _meta: &UpstreamResponseMeta,
        body: &[u8],
    ) -> Result<OAuthQuotaUsageParse, ProviderError> {
        codex_quota::parse_usage(body).map(OAuthQuotaUsageParse::Complete)
    }

    fn parse_oauth_quota_reset_credits(
        &self,
        body: &[u8],
    ) -> Result<Option<crate::api::OAuthQuotaResetCredits>, ProviderError> {
        codex_quota::parse_reset_credits(body)
    }

    fn oauth_quota_reset_plan(
        &self,
        token: &OAuthTokenMaterial,
        redeem_request_id: &str,
    ) -> Result<Option<OAuthRequestPlan>, ProviderError> {
        codex_quota::reset_plan(token, redeem_request_id).map(Some)
    }

    fn parse_oauth_quota_reset(
        &self,
        body: &[u8],
    ) -> Result<crate::api::OAuthQuotaResetResult, ProviderError> {
        codex_quota::parse_reset_result(body)
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

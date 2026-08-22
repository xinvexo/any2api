use super::{
    headers as codex_headers, import as codex_import, model_catalog as codex_model_catalog,
    oauth as codex_oauth, quota as codex_quota, request as codex_request,
};
use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderKind, RequestBodyEncoding, RequestSpeedTier,
    TransportMode,
};
use any2api_protocol::api::{OpenAiChatCompletionsProfile, ProtocolTargetProfile};
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CredentialHeaders, CredentialTestPlan, EndpointPlan, OAuthAuthorizationCodeProvider,
        OAuthCapabilities, OAuthGrant, OAuthImportedAccount, OAuthLoginFlow,
        OAuthModelCatalogScope, OAuthPrincipalIdentity, OAuthQuotaProvider, OAuthQuotaRejection,
        OAuthQuotaResetProvider, OAuthQuotaUsage, OAuthRefreshRejection, OAuthRequestPlan,
        OAuthRoutingProfile, OAuthRoutingProvider, OAuthTokenMaterial, OAuthTokenProvider,
        ProviderDescriptor, ProviderDriver, ProviderRequestContext, UpstreamResponseMeta,
    },
    credential::api_key,
    upstream_error::openai as openai_error,
};

const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    ProviderKind::Codex,
    &[
        ProtocolOperation::Responses,
        ProtocolOperation::ResponsesCompact,
        ProtocolOperation::AlphaSearch,
        ProtocolOperation::ChatCompletions,
        ProtocolOperation::ImagesGenerations,
        ProtocolOperation::ImagesEdits,
    ],
    Some(
        OAuthCapabilities::new(
            &[
                ProtocolOperation::Responses,
                ProtocolOperation::ResponsesCompact,
                ProtocolOperation::AlphaSearch,
            ],
            OAuthLoginFlow::AuthorizationCodePkce,
        )
        .with_quota()
        .with_quota_reset(),
    ),
    &[TransportMode::Json, TransportMode::Sse],
);

#[derive(Debug)]
pub struct CodexDriver;

impl Default for CodexDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexDriver {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ProviderDriver for CodexDriver {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
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
        base_url: &any2api_domain::ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        if !self.descriptor().supports_api_key_operation(operation) {
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
        _base_url: &any2api_domain::ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        api_key::bearer_credential_headers(secret)
    }

    fn prepare_request_body(
        &self,
        context: ProviderRequestContext<'_>,
        body: Bytes,
    ) -> Result<Bytes, ProviderError> {
        codex_request::prepare(context, body)
    }

    fn request_speed_tier(
        &self,
        context: ProviderRequestContext<'_>,
        requested_speed_tier: Option<RequestSpeedTier>,
    ) -> Option<RequestSpeedTier> {
        (context.upstream_operation == ProtocolOperation::Responses)
            .then_some(requested_speed_tier)
            .flatten()
    }

    fn response_speed_tier(
        &self,
        operation: ProtocolOperation,
        observed_speed_tier: Option<RequestSpeedTier>,
    ) -> Option<RequestSpeedTier> {
        (operation == ProtocolOperation::Responses)
            .then_some(observed_speed_tier)
            .flatten()
    }

    fn prepare_request_headers(
        &self,
        context: ProviderRequestContext<'_>,
    ) -> Result<HeaderMap, ProviderError> {
        codex_headers::request(context)
    }

    fn response_headers(&self, _operation: ProtocolOperation, upstream: &HeaderMap) -> HeaderMap {
        codex_headers::response(upstream)
    }

    fn supports_request_body_encoding(
        &self,
        context: ProviderRequestContext<'_>,
        encoding: RequestBodyEncoding,
    ) -> bool {
        codex_headers::supports_encoding(context, encoding)
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

    fn oauth_authorization_code(&self) -> Option<&dyn OAuthAuthorizationCodeProvider> {
        Some(self)
    }

    fn oauth_token(&self) -> Option<&dyn OAuthTokenProvider> {
        Some(self)
    }

    fn oauth_routing(&self) -> Option<&dyn OAuthRoutingProvider> {
        Some(self)
    }

    fn oauth_quota(&self) -> Option<&dyn OAuthQuotaProvider> {
        Some(self)
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

impl OAuthAuthorizationCodeProvider for CodexDriver {
    fn oauth_redirect_uri(&self) -> &'static str {
        codex_oauth::redirect_uri()
    }

    fn oauth_authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
    ) -> Result<Url, ProviderError> {
        codex_oauth::authorization_url(state, code_challenge)
    }

    fn oauth_authorization_code_token_request(
        &self,
        code: &str,
        _state: &str,
        code_verifier: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        codex_oauth::token_request(OAuthGrant::AuthorizationCode, code, Some(code_verifier))
    }
}

impl OAuthTokenProvider for CodexDriver {
    fn oauth_refresh_token_request(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        codex_oauth::token_request(OAuthGrant::RefreshToken, refresh_token, None)
    }

    fn parse_oauth_token_response(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        codex_oauth::parse_token(body)
    }

    fn parse_oauth_import(
        &self,
        object: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<OAuthImportedAccount>, ProviderError> {
        codex_import::parse(object)
    }

    fn oauth_principal_identity(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Option<OAuthPrincipalIdentity> {
        codex_oauth::principal_identity(token)
    }

    fn classify_oauth_refresh_rejection(
        &self,
        status: StatusCode,
        bounded_body: &[u8],
    ) -> OAuthRefreshRejection {
        codex_oauth::classify_refresh_rejection(status, bounded_body)
    }
}

impl OAuthRoutingProvider for CodexDriver {
    fn oauth_routing_profile(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        codex_oauth::routing_profile(token)
    }

    fn oauth_model_catalog_scope(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthModelCatalogScope, ProviderError> {
        codex_model_catalog::scope(token)
    }

    fn oauth_model_catalog_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        codex_model_catalog::request_plan(token)
    }

    fn parse_oauth_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        codex_model_catalog::parse(bounded_body)
    }

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        _forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        codex_oauth::credential_headers(token)
    }
}

impl OAuthQuotaProvider for CodexDriver {
    fn oauth_quota_query_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<Option<crate::api::OAuthQuotaQueryPlan>, ProviderError> {
        codex_quota::query_plan(token).map(Some)
    }

    fn oauth_quota_cost_unit(&self) -> Option<any2api_domain::QuotaCostUnit> {
        Some(any2api_domain::QuotaCostUnit::CodexCredits)
    }

    fn oauth_request_quota_cost_unit(
        &self,
        context: ProviderRequestContext<'_>,
    ) -> Option<any2api_domain::QuotaCostUnit> {
        (context.oauth && context.upstream_operation == ProtocolOperation::Responses)
            .then_some(any2api_domain::QuotaCostUnit::CodexCredits)
    }

    fn classify_oauth_quota_rejection(
        &self,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> OAuthQuotaRejection {
        codex_quota::classify_quota_rejection(meta, bounded_body)
    }

    fn parse_oauth_quota_usage(
        &self,
        _meta: &UpstreamResponseMeta,
        body: &[u8],
    ) -> Result<OAuthQuotaUsage, ProviderError> {
        codex_quota::parse_usage(body)
    }

    fn reset(&self) -> Option<&dyn OAuthQuotaResetProvider> {
        Some(self)
    }
}

impl OAuthQuotaResetProvider for CodexDriver {
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
}

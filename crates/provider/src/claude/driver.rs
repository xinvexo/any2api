use super::{
    client_version as claude_client_version, error as claude_error, headers as claude_headers,
    import as claude_import, model_catalog as claude_model_catalog, oauth as claude_oauth,
    quota as claude_quota,
};
use std::sync::Arc;

use any2api_domain::{
    ProtocolOperation, ProviderBaseUrl, ProviderKind, RequestSpeedTier, TransportMode,
};
use arc_swap::ArcSwapOption;
use http::{HeaderMap, HeaderValue};
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CredentialHeaders, CredentialTestPlan, EndpointPlan, OAuthAuthorizationCodeProvider,
        OAuthCapabilities, OAuthGrant, OAuthImportedAccount, OAuthLoginFlow,
        OAuthModelCatalogScope, OAuthQuotaProvider, OAuthQuotaQueryPlan, OAuthQuotaUsage,
        OAuthRequestPlan, OAuthRoutingProfile, OAuthRoutingProvider, OAuthTokenMaterial,
        OAuthTokenProvider, OfficialClientVersion, OfficialClientVersionProvider,
        OfficialClientVersionRequestPlan, ProviderDescriptor, ProviderDriver,
        ProviderRequestContext, UpstreamResponseMeta,
    },
    credential::api_key,
};

const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    ProviderKind::Claude,
    &[
        ProtocolOperation::Messages,
        ProtocolOperation::MessagesCountTokens,
    ],
    Some(
        OAuthCapabilities::new(
            &[
                ProtocolOperation::Messages,
                ProtocolOperation::MessagesCountTokens,
            ],
            OAuthLoginFlow::AuthorizationCodePkce,
        )
        .with_quota(),
    ),
    &[TransportMode::Json, TransportMode::Sse],
);

#[derive(Debug)]
pub struct ClaudeDriver {
    official_client_version: ArcSwapOption<OfficialClientVersion>,
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
            official_client_version: ArcSwapOption::empty(),
        }
    }

    #[must_use]
    pub fn new_with_official_client_version(version: OfficialClientVersion) -> Self {
        Self {
            official_client_version: ArcSwapOption::from(Some(Arc::new(version))),
        }
    }
}

impl ProviderDriver for ClaudeDriver {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
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
        let version = self.require_official_client_version()?;
        claude_headers::request(context, &version)
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

    fn official_client_version(&self) -> Option<&dyn OfficialClientVersionProvider> {
        Some(self)
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

impl OfficialClientVersionProvider for ClaudeDriver {
    fn latest_official_client_version_plan(
        &self,
    ) -> Result<OfficialClientVersionRequestPlan, ProviderError> {
        claude_client_version::request_plan()
    }

    fn parse_latest_official_client_version(
        &self,
        bounded_body: &[u8],
    ) -> Result<OfficialClientVersion, ProviderError> {
        claude_client_version::parse(bounded_body)
    }

    fn current_official_client_version(&self) -> Option<Arc<OfficialClientVersion>> {
        self.official_client_version.load_full()
    }

    fn publish_official_client_version(&self, version: OfficialClientVersion) {
        self.official_client_version.store(Some(Arc::new(version)));
    }
}

impl OAuthAuthorizationCodeProvider for ClaudeDriver {
    fn oauth_redirect_uri(&self) -> &'static str {
        claude_oauth::redirect_uri()
    }

    fn oauth_authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
    ) -> Result<Url, ProviderError> {
        claude_oauth::authorization_url(state, code_challenge)
    }

    fn oauth_authorization_code_token_request(
        &self,
        code: &str,
        state: &str,
        code_verifier: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        claude_oauth::token_request(
            OAuthGrant::AuthorizationCode,
            code,
            Some(state),
            Some(code_verifier),
        )
    }
}

impl OAuthTokenProvider for ClaudeDriver {
    fn oauth_refresh_token_request(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        claude_oauth::token_request(OAuthGrant::RefreshToken, refresh_token, None, None)
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
}

impl OAuthRoutingProvider for ClaudeDriver {
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

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        claude_oauth::credential_headers(token, forwarded)
    }
}

impl OAuthQuotaProvider for ClaudeDriver {
    fn oauth_quota_query_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<Option<OAuthQuotaQueryPlan>, ProviderError> {
        let version = self.require_official_client_version()?;
        claude_quota::query_plan(token, &version).map(Some)
    }

    fn parse_oauth_quota_usage(
        &self,
        _meta: &UpstreamResponseMeta,
        body: &[u8],
    ) -> Result<OAuthQuotaUsage, ProviderError> {
        claude_quota::parse_usage(body)
    }
}

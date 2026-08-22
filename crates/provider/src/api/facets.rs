use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, QuotaCostUnit,
    RequestBodyEncoding, RequestSpeedTier, UpstreamError,
};
use any2api_protocol::api::{OpenAiChatCompletionsProfile, ProtocolTargetProfile};
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use url::Url;

use super::{
    CredentialHeaders, CredentialTestPlan, EndpointPlan, OAuthDeviceAuthorization,
    OAuthDeviceTokenPoll, OAuthImportedAccount, OAuthModelCatalogScope, OAuthPrincipalIdentity,
    OAuthQuotaQueryPlan, OAuthQuotaRejection, OAuthQuotaResetCredits, OAuthQuotaResetResult,
    OAuthQuotaSupplement, OAuthQuotaUsage, OAuthRefreshRejection, OAuthRequestPlan,
    OAuthRoutingProfile, OAuthTokenMaterial, ProviderDescriptor, ProviderError,
    ProviderRequestContext, ProviderSecret, UpstreamResponseMeta,
};

pub trait ProviderDriver: Send + Sync {
    fn descriptor(&self) -> &'static ProviderDescriptor;

    fn kind(&self) -> ProviderKind {
        self.descriptor().kind()
    }

    fn protocol_target_profile(
        &self,
        dialect: ProtocolDialect,
        _upstream_model: &str,
    ) -> Option<ProtocolTargetProfile> {
        (dialect == ProtocolDialect::OpenAiChatCompletions
            && self.descriptor().supports_protocol(dialect))
        .then_some(ProtocolTargetProfile::OpenAiChatCompletions(
            OpenAiChatCompletionsProfile::COMPATIBLE_BASELINE,
        ))
    }

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError>;

    fn endpoint_plan(
        &self,
        base_url: &ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError>;

    fn credential_test_plan(
        &self,
        base_url: &ProviderBaseUrl,
    ) -> Result<CredentialTestPlan, ProviderError>;

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError>;

    fn credential_headers(
        &self,
        base_url: &ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError>;

    fn request_speed_tier(
        &self,
        _context: ProviderRequestContext<'_>,
        _requested_speed_tier: Option<RequestSpeedTier>,
    ) -> Option<RequestSpeedTier> {
        None
    }

    fn response_speed_tier(
        &self,
        _operation: ProtocolOperation,
        _observed_speed_tier: Option<RequestSpeedTier>,
    ) -> Option<RequestSpeedTier> {
        None
    }

    fn prepare_request_body(
        &self,
        _context: ProviderRequestContext<'_>,
        body: Bytes,
    ) -> Result<Bytes, ProviderError> {
        Ok(body)
    }

    fn prepare_request_headers(
        &self,
        _context: ProviderRequestContext<'_>,
    ) -> Result<HeaderMap, ProviderError> {
        Ok(HeaderMap::new())
    }

    fn response_headers(&self, _operation: ProtocolOperation, _upstream: &HeaderMap) -> HeaderMap {
        HeaderMap::new()
    }

    fn supports_request_body_encoding(
        &self,
        _context: ProviderRequestContext<'_>,
        _encoding: RequestBodyEncoding,
    ) -> bool {
        false
    }

    fn oauth_authorization_code(&self) -> Option<&dyn OAuthAuthorizationCodeProvider> {
        None
    }

    fn oauth_device_code(&self) -> Option<&dyn OAuthDeviceCodeProvider> {
        None
    }

    fn oauth_token(&self) -> Option<&dyn OAuthTokenProvider> {
        None
    }

    fn oauth_routing(&self) -> Option<&dyn OAuthRoutingProvider> {
        None
    }

    fn oauth_quota(&self) -> Option<&dyn OAuthQuotaProvider> {
        None
    }

    fn classify_error(
        &self,
        operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> UpstreamError;
}

pub trait OAuthAuthorizationCodeProvider: ProviderDriver {
    fn oauth_redirect_uri(&self) -> &'static str;

    fn oauth_authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
    ) -> Result<Url, ProviderError>;

    fn oauth_authorization_code_token_request(
        &self,
        code: &str,
        state: &str,
        code_verifier: &str,
    ) -> Result<OAuthRequestPlan, ProviderError>;
}

pub trait OAuthDeviceCodeProvider: ProviderDriver {
    fn oauth_device_authorization_request(&self) -> Result<OAuthRequestPlan, ProviderError>;

    fn parse_oauth_device_authorization(
        &self,
        body: &[u8],
    ) -> Result<OAuthDeviceAuthorization, ProviderError>;

    fn oauth_device_token_request(
        &self,
        device_code: &str,
    ) -> Result<OAuthRequestPlan, ProviderError>;

    fn parse_oauth_device_token(
        &self,
        status: StatusCode,
        body: &[u8],
    ) -> Result<OAuthDeviceTokenPoll, ProviderError>;
}

pub trait OAuthTokenProvider: ProviderDriver {
    fn oauth_refresh_token_request(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthRequestPlan, ProviderError>;

    fn parse_oauth_token_response(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError>;

    fn parse_oauth_import(
        &self,
        object: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<OAuthImportedAccount>, ProviderError>;

    fn parse_oauth_refresh_response(
        &self,
        body: &[u8],
        previous: &OAuthTokenMaterial,
    ) -> Result<OAuthTokenMaterial, ProviderError> {
        self.parse_oauth_token_response(body)?
            .merge_refresh_response(previous)
    }

    fn oauth_principal_identity(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Option<OAuthPrincipalIdentity> {
        (token.provider() == self.kind())
            .then(|| crate::oauth::default_principal_identity(token))
            .flatten()
    }

    fn classify_oauth_refresh_rejection(
        &self,
        status: StatusCode,
        bounded_body: &[u8],
    ) -> OAuthRefreshRejection {
        OAuthRefreshRejection::classify(status, bounded_body)
    }
}

pub trait OAuthRoutingProvider: ProviderDriver {
    fn oauth_routing_profile(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError>;

    fn oauth_model_catalog_scope(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthModelCatalogScope, ProviderError>;

    fn oauth_model_catalog_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthRequestPlan, ProviderError>;

    fn parse_oauth_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError>;

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError>;
}

pub trait OAuthQuotaProvider: ProviderDriver {
    fn oauth_quota_query_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<Option<OAuthQuotaQueryPlan>, ProviderError>;

    fn oauth_quota_cost_unit(&self) -> Option<QuotaCostUnit> {
        None
    }

    fn oauth_request_quota_cost_unit(
        &self,
        _context: ProviderRequestContext<'_>,
    ) -> Option<QuotaCostUnit> {
        None
    }

    fn classify_oauth_quota_rejection(
        &self,
        _meta: &UpstreamResponseMeta,
        _bounded_body: &[u8],
    ) -> OAuthQuotaRejection {
        OAuthQuotaRejection::Unclassified
    }

    fn parse_oauth_quota_usage(
        &self,
        meta: &UpstreamResponseMeta,
        body: &[u8],
    ) -> Result<OAuthQuotaUsage, ProviderError>;

    fn supplement(&self) -> Option<&dyn OAuthQuotaSupplementProvider> {
        None
    }

    fn reset(&self) -> Option<&dyn OAuthQuotaResetProvider> {
        None
    }
}

pub trait OAuthQuotaSupplementProvider: OAuthQuotaProvider {
    fn parse_oauth_quota_supplement(
        &self,
        body: &[u8],
    ) -> Result<OAuthQuotaSupplement, ProviderError>;
}

pub trait OAuthQuotaResetProvider: OAuthQuotaProvider {
    fn parse_oauth_quota_reset_credits(
        &self,
        body: &[u8],
    ) -> Result<Option<OAuthQuotaResetCredits>, ProviderError>;

    fn oauth_quota_reset_plan(
        &self,
        token: &OAuthTokenMaterial,
        redeem_request_id: &str,
    ) -> Result<Option<OAuthRequestPlan>, ProviderError>;

    fn parse_oauth_quota_reset(&self, body: &[u8]) -> Result<OAuthQuotaResetResult, ProviderError>;
}

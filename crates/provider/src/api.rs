use std::{collections::BTreeSet, fmt};

use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind,
    QuotaCostUnit, QuotaServiceTier, RequestBodyEncoding, TransportMode, UpstreamError,
};
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use url::Url;

pub use crate::codex::oauth_plan_label as codex_oauth_plan_label;
pub use crate::credential::ProviderSecret;
pub use crate::error::ProviderError;
pub use crate::grok::oauth_bot_flag as grok_oauth_bot_flag;
pub use crate::oauth::OAuthRoutingProfile;
pub use crate::oauth::{
    MAX_OAUTH_IMPORT_ACCOUNTS_PER_DOCUMENT, OAuthImportParseError, OAuthImportedAccount,
    parse_oauth_import_document,
};
pub use crate::oauth::{
    OAuthDeviceAuthorization, OAuthDeviceTokenPoll, OAuthGrant, OAuthLoginFlow,
    OAuthRefreshRejection, OAuthRequestPlan, OAuthTokenMaterial, decode_oauth_account_document,
    encode_oauth_account_document,
};
pub use crate::oauth::{
    OAuthQuotaAccessStatus, OAuthQuotaAccountStatus, OAuthQuotaAuthenticationStatus,
    OAuthQuotaBilling, OAuthQuotaCostRate, OAuthQuotaCredits, OAuthQuotaExhaustion,
    OAuthQuotaQueryPlan, OAuthQuotaRateLimit, OAuthQuotaReachedType, OAuthQuotaRejection,
    OAuthQuotaResetCredit, OAuthQuotaResetCredits, OAuthQuotaResetResult, OAuthQuotaSupplement,
    OAuthQuotaTokenBalance, OAuthQuotaTokenBalanceSource, OAuthQuotaUsage, OAuthQuotaWindow,
    OAuthQuotaWindowKind,
};
pub use crate::registry::ProviderRegistry;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapabilitySet {
    pub protocols: BTreeSet<ProtocolDialect>,
    pub transport_modes: BTreeSet<TransportMode>,
    pub credential_kinds: BTreeSet<CredentialKind>,
}

#[derive(Clone, Debug)]
pub struct EndpointPlan {
    pub url: Url,
}

#[derive(Clone)]
pub struct CredentialTestPlan {
    pub url: Url,
    pub headers: HeaderMap,
}

#[derive(Clone, Default)]
pub struct CredentialHeaders {
    pub headers: HeaderMap,
}

#[derive(Clone)]
pub struct UpstreamResponseMeta {
    pub status: StatusCode,
    pub headers: HeaderMap,
}

#[derive(Clone, Copy)]
pub struct ProviderRequestContext<'a> {
    pub ingress_dialect: ProtocolDialect,
    pub upstream_operation: ProtocolOperation,
    pub upstream_model: &'a str,
    pub client_headers: &'a HeaderMap,
    pub oauth: bool,
    pub allow_credential_bound: bool,
    pub allow_turn_state: bool,
}

pub trait ProviderDriver: Send + Sync {
    fn kind(&self) -> ProviderKind;

    fn capabilities(&self) -> &CapabilitySet;

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

    fn oauth_login_flow(&self) -> Option<OAuthLoginFlow> {
        None
    }

    fn oauth_redirect_uri(&self) -> Option<&'static str> {
        None
    }

    fn oauth_authorization_url(
        &self,
        _state: &str,
        _code_challenge: &str,
    ) -> Result<Url, ProviderError> {
        Err(ProviderError::InvalidCredential(
            "OAuth2 is not supported by this provider".into(),
        ))
    }

    fn oauth_token_request(
        &self,
        _grant: OAuthGrant,
        _code: &str,
        _state: Option<&str>,
        _code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        Err(ProviderError::InvalidCredential(
            "OAuth2 is not supported by this provider".into(),
        ))
    }

    fn oauth_device_authorization_request(&self) -> Result<OAuthRequestPlan, ProviderError> {
        Err(ProviderError::InvalidCredential(
            "OAuth device authorization is not supported by this provider".into(),
        ))
    }

    fn parse_oauth_device_authorization(
        &self,
        _body: &[u8],
    ) -> Result<OAuthDeviceAuthorization, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth device authorization is not supported by this provider".into(),
        ))
    }

    fn oauth_device_token_request(
        &self,
        _device_code: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        Err(ProviderError::InvalidCredential(
            "OAuth device authorization is not supported by this provider".into(),
        ))
    }

    fn parse_oauth_device_token(
        &self,
        _status: StatusCode,
        _body: &[u8],
    ) -> Result<OAuthDeviceTokenPoll, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth device authorization is not supported by this provider".into(),
        ))
    }

    fn parse_oauth_token_response(
        &self,
        _body: &[u8],
    ) -> Result<OAuthTokenMaterial, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth2 is not supported by this provider".into(),
        ))
    }

    fn parse_oauth_import(
        &self,
        _object: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<OAuthImportedAccount>, ProviderError> {
        Ok(None)
    }

    fn parse_oauth_refresh_response(
        &self,
        body: &[u8],
        previous: &OAuthTokenMaterial,
    ) -> Result<OAuthTokenMaterial, ProviderError> {
        self.parse_oauth_token_response(body)?
            .merge_refresh_response(previous)
    }

    fn classify_oauth_refresh_rejection(
        &self,
        status: StatusCode,
        bounded_body: &[u8],
    ) -> OAuthRefreshRejection {
        OAuthRefreshRejection::classify(status, bounded_body)
    }

    fn oauth_routing_profile(
        &self,
        _token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth2 is not supported by this provider".into(),
        ))
    }

    fn oauth_supports_operation(&self, _operation: ProtocolOperation) -> bool {
        false
    }

    fn oauth_credential_headers(
        &self,
        _token: &OAuthTokenMaterial,
        _forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        Err(ProviderError::InvalidCredential(
            "OAuth2 is not supported by this provider".into(),
        ))
    }

    fn oauth_quota_query_plan(
        &self,
        _token: &OAuthTokenMaterial,
    ) -> Result<Option<OAuthQuotaQueryPlan>, ProviderError> {
        Ok(None)
    }

    fn oauth_quota_cost_rate(
        &self,
        _model: &str,
        _service_tier: QuotaServiceTier,
    ) -> Option<OAuthQuotaCostRate> {
        None
    }

    fn oauth_quota_cost_unit(&self) -> Option<QuotaCostUnit> {
        None
    }

    fn oauth_quota_service_tier(
        &self,
        _context: ProviderRequestContext<'_>,
        _prepared_body: &[u8],
    ) -> Option<QuotaServiceTier> {
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
        _meta: &UpstreamResponseMeta,
        _body: &[u8],
    ) -> Result<OAuthQuotaUsage, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth quota is not supported by this provider".into(),
        ))
    }

    fn parse_oauth_quota_supplement(
        &self,
        _body: &[u8],
    ) -> Result<OAuthQuotaSupplement, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth quota supplement is not supported by this provider".into(),
        ))
    }

    fn parse_oauth_quota_reset_credits(
        &self,
        _body: &[u8],
    ) -> Result<Option<OAuthQuotaResetCredits>, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth quota is not supported by this provider".into(),
        ))
    }

    fn oauth_quota_reset_plan(
        &self,
        _token: &OAuthTokenMaterial,
        _redeem_request_id: &str,
    ) -> Result<Option<OAuthRequestPlan>, ProviderError> {
        Ok(None)
    }

    fn parse_oauth_quota_reset(
        &self,
        _body: &[u8],
    ) -> Result<OAuthQuotaResetResult, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth quota reset is not supported by this provider".into(),
        ))
    }

    fn classify_error(
        &self,
        operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> UpstreamError;
}

impl fmt::Debug for CredentialHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialHeaders")
            .field("header_count", &self.headers.len())
            .finish()
    }
}

impl fmt::Debug for UpstreamResponseMeta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamResponseMeta")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .finish()
    }
}

impl fmt::Debug for ProviderRequestContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRequestContext")
            .field("ingress_dialect", &self.ingress_dialect)
            .field("upstream_operation", &self.upstream_operation)
            .field("upstream_model", &self.upstream_model)
            .field("client_header_count", &self.client_headers.len())
            .field("oauth", &self.oauth)
            .field("allow_credential_bound", &self.allow_credential_bound)
            .field("allow_turn_state", &self.allow_turn_state)
            .finish()
    }
}

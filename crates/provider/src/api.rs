use std::fmt;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use http::{HeaderMap, StatusCode};
use url::Url;

mod descriptor;
mod facets;

pub use crate::codex::oauth_plan_label as codex_oauth_plan_label;
pub use crate::credential::ProviderSecret;
pub use crate::error::ProviderError;
pub use crate::grok::oauth_bot_flag as grok_oauth_bot_flag;
pub use crate::oauth::{
    MAX_OAUTH_IMPORT_ACCOUNTS_PER_DOCUMENT, OAuthImportParseError, OAuthImportedAccount,
    parse_oauth_import_document,
};
pub use crate::oauth::{
    OAuthDeviceAuthorization, OAuthDeviceTokenPoll, OAuthGrant, OAuthLoginFlow,
    OAuthPrincipalIdentity, OAuthRefreshRejection, OAuthRequestPlan, OAuthTokenMaterial,
    decode_oauth_account_document, encode_oauth_account_document,
};
pub use crate::oauth::{OAuthModelCatalogScope, OAuthRoutingProfile};
pub use crate::oauth::{
    OAuthQuotaAccessStatus, OAuthQuotaAccountStatus, OAuthQuotaAuthenticationStatus,
    OAuthQuotaBilling, OAuthQuotaCredits, OAuthQuotaExhaustion, OAuthQuotaQueryPlan,
    OAuthQuotaRateLimit, OAuthQuotaReachedType, OAuthQuotaRejection, OAuthQuotaResetCredit,
    OAuthQuotaResetCredits, OAuthQuotaResetResult, OAuthQuotaSupplement, OAuthQuotaTokenBalance,
    OAuthQuotaTokenBalanceSource, OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind,
};
pub use crate::registry::ProviderRegistry;
pub use descriptor::{OAuthCapabilities, ProviderDescriptor};
pub use facets::{
    OAuthAuthorizationCodeProvider, OAuthDeviceCodeProvider, OAuthQuotaProvider,
    OAuthQuotaResetProvider, OAuthQuotaSupplementProvider, OAuthRoutingProvider,
    OAuthTokenProvider, ProviderDriver,
};

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
    pub allow_session_replay: bool,
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
            .field("allow_session_replay", &self.allow_session_replay)
            .finish()
    }
}

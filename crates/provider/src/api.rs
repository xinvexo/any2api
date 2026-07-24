use std::{collections::BTreeSet, fmt};

use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind,
    TransportMode, UpstreamErrorClassification,
};
use http::{HeaderMap, StatusCode};
use url::Url;

pub use crate::oauth::{OAuthGrant, OAuthRequestPlan, OAuthTokenMaterial, serialize_file};
pub use crate::oauth_routing::OAuthRoutingProfile;
pub use crate::{ProviderError, ProviderRegistry, ProviderSecret};

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

#[derive(Clone, Default)]
pub struct CredentialHeaders {
    pub headers: HeaderMap,
}

#[derive(Clone)]
pub struct UpstreamResponseMeta {
    pub status: StatusCode,
    pub headers: HeaderMap,
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
    ) -> Result<EndpointPlan, ProviderError>;

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError>;

    fn credential_headers(
        &self,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError>;

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

    fn parse_oauth_token(&self, _body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth2 is not supported by this provider".into(),
        ))
    }

    fn parse_oauth_refresh_token(
        &self,
        body: &[u8],
        previous: &OAuthTokenMaterial,
    ) -> Result<OAuthTokenMaterial, ProviderError> {
        self.parse_oauth_token(body)?
            .with_refresh_fallbacks(previous)
    }

    fn oauth_routing_profile(
        &self,
        _token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        Err(ProviderError::InvalidResponse(
            "OAuth2 is not supported by this provider".into(),
        ))
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

    fn classify_error(
        &self,
        operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> UpstreamErrorClassification;
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

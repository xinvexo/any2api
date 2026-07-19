use std::{collections::BTreeSet, fmt};

use any2api_domain::{
    CredentialKind, ErrorClass, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind,
    TransportMode,
};
use http::{HeaderMap, StatusCode};
use url::Url;

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

    fn credential_headers(
        &self,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError>;

    fn classify_error(
        &self,
        operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> ErrorClass;
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

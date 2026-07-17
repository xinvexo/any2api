use std::collections::BTreeSet;

use any2api_domain::{CredentialKind, ErrorClass, ProtocolDialect, ProviderKind, TransportMode};
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
    pub base_url: Url,
}

#[derive(Clone, Debug, Default)]
pub struct CredentialHeaders {
    pub headers: HeaderMap,
}

#[derive(Clone, Debug)]
pub struct UpstreamResponseMeta {
    pub status: StatusCode,
    pub headers: HeaderMap,
}

pub trait ProviderDriver: Send + Sync {
    fn kind(&self) -> ProviderKind;

    fn capabilities(&self) -> &CapabilitySet;

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError>;

    fn endpoint_plan(&self, base_url: &Url) -> Result<EndpointPlan, ProviderError>;

    fn credential_headers(
        &self,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError>;

    fn classify_error(&self, meta: &UpstreamResponseMeta, bounded_body: &[u8]) -> ErrorClass;
}

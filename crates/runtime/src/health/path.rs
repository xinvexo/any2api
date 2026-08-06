use any2api_domain::{
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, RouteTargetId, RoutingCredentialId,
};

/// Stable identity for an actual endpoint/egress combination. Versions keep
/// a changed URL, proxy address, or proxy authentication from inheriting old
/// path health.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct EgressPathKey {
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) endpoint_config_version: u64,
    pub(crate) proxy_id: ProxyProfileId,
    pub(crate) proxy_config_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CandidatePathBaseKey {
    pub(crate) target_id: RouteTargetId,
    pub(crate) credential_id: RoutingCredentialId,
    pub(crate) routing_generation: u64,
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) endpoint_config_version: u64,
    pub(crate) proxy_id: ProxyProfileId,
    pub(crate) proxy_config_version: u64,
}

impl CandidatePathBaseKey {
    pub(crate) const fn egress_path(self) -> EgressPathKey {
        EgressPathKey {
            endpoint_id: self.endpoint_id,
            endpoint_config_version: self.endpoint_config_version,
            proxy_id: self.proxy_id,
            proxy_config_version: self.proxy_config_version,
        }
    }
}

/// Stable identity for one route candidate and operation. The routing
/// generation changes when an API key/account identity changes; an OAuth
/// token refresh intentionally keeps the routing generation and its quota
/// observations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CandidatePathKey {
    pub(crate) target_id: RouteTargetId,
    pub(crate) operation: ProtocolOperation,
    pub(crate) credential_id: RoutingCredentialId,
    pub(crate) routing_generation: u64,
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) endpoint_config_version: u64,
    pub(crate) proxy_id: ProxyProfileId,
    pub(crate) proxy_config_version: u64,
}

impl CandidatePathKey {
    pub(crate) const fn base(self) -> CandidatePathBaseKey {
        CandidatePathBaseKey {
            target_id: self.target_id,
            credential_id: self.credential_id,
            routing_generation: self.routing_generation,
            endpoint_id: self.endpoint_id,
            endpoint_config_version: self.endpoint_config_version,
            proxy_id: self.proxy_id,
            proxy_config_version: self.proxy_config_version,
        }
    }
}

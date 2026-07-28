use any2api_domain::{ProtocolDialect, RouteTargetId, RoutingCredentialId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinityBindingSummary {
    pub(crate) session_hash_prefix: String,
    pub(crate) credential_id: RoutingCredentialId,
    pub(crate) route_target_id: RouteTargetId,
    pub(crate) upstream_model: String,
    pub(crate) protocol_dialect: ProtocolDialect,
    pub(crate) expires_in_ms: u64,
}

impl AffinityBindingSummary {
    pub fn session_hash_prefix(&self) -> &str {
        &self.session_hash_prefix
    }

    pub const fn credential_id(&self) -> RoutingCredentialId {
        self.credential_id
    }

    pub const fn route_target_id(&self) -> RouteTargetId {
        self.route_target_id
    }

    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    pub const fn protocol_dialect(&self) -> ProtocolDialect {
        self.protocol_dialect
    }

    pub const fn expires_in_ms(&self) -> u64 {
        self.expires_in_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinityCredentialCount {
    pub(crate) credential_id: RoutingCredentialId,
    pub(crate) bindings: usize,
}

impl AffinityCredentialCount {
    pub const fn credential_id(&self) -> RoutingCredentialId {
        self.credential_id
    }

    pub const fn bindings(&self) -> usize {
        self.bindings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinityRuntimeSnapshot {
    pub(crate) binding_count: usize,
    pub(crate) creating_count: usize,
    pub(crate) credential_counts: Vec<AffinityCredentialCount>,
    pub(crate) bindings: Vec<AffinityBindingSummary>,
}

impl AffinityRuntimeSnapshot {
    pub const fn binding_count(&self) -> usize {
        self.binding_count
    }

    pub const fn creating_count(&self) -> usize {
        self.creating_count
    }

    pub fn credential_counts(&self) -> &[AffinityCredentialCount] {
        &self.credential_counts
    }

    pub fn bindings(&self) -> &[AffinityBindingSummary] {
        &self.bindings
    }
}

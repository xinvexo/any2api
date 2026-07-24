use any2api_domain::{ProtocolDialect, RouteTargetId, RoutingCredentialId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AffinityBindingKind {
    Soft,
    Hard,
}

impl AffinityBindingKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Hard => "hard",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinityBindingSummary {
    pub(crate) kind: AffinityBindingKind,
    pub(crate) session_hash_prefix: String,
    pub(crate) credential_id: RoutingCredentialId,
    pub(crate) route_target_id: RouteTargetId,
    pub(crate) upstream_model: String,
    pub(crate) protocol_dialect: ProtocolDialect,
    pub(crate) expires_in_ms: u64,
}

impl AffinityBindingSummary {
    pub const fn kind(&self) -> AffinityBindingKind {
        self.kind
    }

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
    pub(crate) soft_bindings: usize,
    pub(crate) hard_bindings: usize,
}

impl AffinityCredentialCount {
    pub const fn credential_id(&self) -> RoutingCredentialId {
        self.credential_id
    }

    pub const fn soft_bindings(&self) -> usize {
        self.soft_bindings
    }

    pub const fn hard_bindings(&self) -> usize {
        self.hard_bindings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinityRuntimeSnapshot {
    pub(crate) soft_binding_count: usize,
    pub(crate) hard_binding_count: usize,
    pub(crate) creating_count: usize,
    pub(crate) credential_counts: Vec<AffinityCredentialCount>,
    pub(crate) bindings: Vec<AffinityBindingSummary>,
}

impl AffinityRuntimeSnapshot {
    pub const fn soft_binding_count(&self) -> usize {
        self.soft_binding_count
    }

    pub const fn hard_binding_count(&self) -> usize {
        self.hard_binding_count
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

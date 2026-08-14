use any2api_domain::{
    ConfigRevision, ProtocolDialect, ProtocolOperation, ProviderEndpointId, ProviderKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteInspectionSnapshot {
    pub(super) config_revision: ConfigRevision,
    pub(super) items: Vec<RouteInspectionItem>,
}

impl RouteInspectionSnapshot {
    #[must_use]
    pub const fn config_revision(&self) -> ConfigRevision {
        self.config_revision
    }

    #[must_use]
    pub fn items(&self) -> &[RouteInspectionItem] {
        &self.items
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteInspectionItem {
    pub(super) public_model: String,
    pub(super) ingress_protocol: ProtocolDialect,
    pub(super) allowed: bool,
    pub(super) published: bool,
    pub(super) status: RouteInspectionStatus,
    pub(super) operations: Vec<RouteInspectionOperation>,
}

impl RouteInspectionItem {
    #[must_use]
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    #[must_use]
    pub const fn ingress_protocol(&self) -> ProtocolDialect {
        self.ingress_protocol
    }

    #[must_use]
    pub const fn allowed(&self) -> bool {
        self.allowed
    }

    #[must_use]
    pub const fn published(&self) -> bool {
        self.published
    }

    #[must_use]
    pub const fn status(&self) -> RouteInspectionStatus {
        self.status
    }

    #[must_use]
    pub fn operations(&self) -> &[RouteInspectionOperation] {
        &self.operations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteInspectionStatus {
    Available,
    BlockedByPolicy,
    NoEnabledCandidate,
}

impl RouteInspectionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::BlockedByPolicy => "blocked_by_policy",
            Self::NoEnabledCandidate => "no_enabled_candidate",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteInspectionOperation {
    pub(super) operation: ProtocolOperation,
    pub(super) candidate_groups: Vec<RouteInspectionCandidateGroup>,
}

impl RouteInspectionOperation {
    #[must_use]
    pub const fn operation(&self) -> ProtocolOperation {
        self.operation
    }

    #[must_use]
    pub fn candidate_groups(&self) -> &[RouteInspectionCandidateGroup] {
        &self.candidate_groups
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteInspectionCandidateGroup {
    pub(super) provider_kind: ProviderKind,
    pub(super) provider_endpoint_id: Option<ProviderEndpointId>,
    pub(super) provider_endpoint_name: Option<String>,
    pub(super) upstream_protocol_dialect: ProtocolDialect,
    pub(super) enabled_candidate_count: usize,
}

impl RouteInspectionCandidateGroup {
    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub const fn provider_endpoint_id(&self) -> Option<ProviderEndpointId> {
        self.provider_endpoint_id
    }

    #[must_use]
    pub fn provider_endpoint_name(&self) -> Option<&str> {
        self.provider_endpoint_name.as_deref()
    }

    #[must_use]
    pub const fn upstream_protocol_dialect(&self) -> ProtocolDialect {
        self.upstream_protocol_dialect
    }

    #[must_use]
    pub const fn enabled_candidate_count(&self) -> usize {
        self.enabled_candidate_count
    }
}

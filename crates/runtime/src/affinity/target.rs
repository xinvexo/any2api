use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect, RouteTargetId};

use crate::route_candidates::RouteCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AffinityTarget {
    route_id: ModelRouteId,
    target_id: RouteTargetId,
    credential_id: CredentialId,
    upstream_model: String,
    protocol_dialect: ProtocolDialect,
}

impl AffinityTarget {
    pub(crate) fn new(
        route_id: ModelRouteId,
        target_id: RouteTargetId,
        credential_id: CredentialId,
        upstream_model: impl Into<String>,
        protocol_dialect: ProtocolDialect,
    ) -> Self {
        Self {
            route_id,
            target_id,
            credential_id,
            upstream_model: upstream_model.into(),
            protocol_dialect,
        }
    }

    pub(crate) fn from_candidate(
        route_id: ModelRouteId,
        protocol_dialect: ProtocolDialect,
        candidate: &RouteCandidate,
    ) -> Self {
        Self::new(
            route_id,
            candidate.target_id,
            candidate.credential_id,
            candidate.upstream_model.clone(),
            protocol_dialect,
        )
    }

    pub(crate) fn matches_candidate(
        &self,
        route_id: ModelRouteId,
        protocol_dialect: ProtocolDialect,
        candidate: &RouteCandidate,
    ) -> bool {
        self.route_id == route_id
            && self.protocol_dialect == protocol_dialect
            && self.target_id == candidate.target_id
            && self.credential_id == candidate.credential_id
            && self.upstream_model == candidate.upstream_model
    }

    pub(crate) const fn target_id(&self) -> RouteTargetId {
        self.target_id
    }

    pub(crate) const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }

    pub(crate) fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    pub(crate) const fn protocol_dialect(&self) -> ProtocolDialect {
        self.protocol_dialect
    }
}

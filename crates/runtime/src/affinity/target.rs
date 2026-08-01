use any2api_domain::{ModelRouteId, ProtocolDialect, RouteTargetId, RoutingCredentialId};

use crate::routing::RouteCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AffinityTarget {
    route_id: ModelRouteId,
    target_id: RouteTargetId,
    credential_id: RoutingCredentialId,
    upstream_model: String,
    ingress_protocol_dialect: ProtocolDialect,
    upstream_protocol_dialect: ProtocolDialect,
}

impl AffinityTarget {
    pub(crate) fn new(
        route_id: ModelRouteId,
        target_id: RouteTargetId,
        credential_id: RoutingCredentialId,
        upstream_model: impl Into<String>,
        ingress_protocol_dialect: ProtocolDialect,
        upstream_protocol_dialect: ProtocolDialect,
    ) -> Self {
        Self {
            route_id,
            target_id,
            credential_id,
            upstream_model: upstream_model.into(),
            ingress_protocol_dialect,
            upstream_protocol_dialect,
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
            candidate.upstream_protocol_dialect,
        )
    }

    pub(crate) fn matches_candidate(
        &self,
        route_id: ModelRouteId,
        protocol_dialect: ProtocolDialect,
        candidate: &RouteCandidate,
    ) -> bool {
        self.route_id == route_id
            && self.ingress_protocol_dialect == protocol_dialect
            && self.upstream_protocol_dialect == candidate.upstream_protocol_dialect
            && self.target_id == candidate.target_id
            && self.credential_id == candidate.credential_id
            && self.upstream_model == candidate.upstream_model
    }

    pub(crate) const fn credential_id(&self) -> RoutingCredentialId {
        self.credential_id
    }
}

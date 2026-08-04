use std::time::Duration;

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect, RouteTargetId};

use super::{AffinityRegistry, AffinityTarget, ContinuationLookup};

mod capacity;
mod continuation;
mod creation;
mod credentials;
mod inspection;

const TTL: Duration = Duration::from_secs(120);

fn target(route_id: ModelRouteId, credential_id: CredentialId) -> AffinityTarget {
    AffinityTarget::new(
        route_id,
        RouteTargetId::new(),
        credential_id.into(),
        "upstream-model",
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiResponses,
    )
}

fn resolved_continuation_target(
    registry: &AffinityRegistry,
    raw: &str,
    ttl: Duration,
) -> Option<AffinityTarget> {
    match registry.resolve_continuation(raw, ttl, |_| true) {
        ContinuationLookup::Ready(resolved) => Some(resolved.into_parts().0),
        ContinuationLookup::Missing | ContinuationLookup::Pending => None,
    }
}

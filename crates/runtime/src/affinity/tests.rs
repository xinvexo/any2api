use std::time::Duration;

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect, RouteTargetId};

use super::AffinityTarget;

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
    )
}

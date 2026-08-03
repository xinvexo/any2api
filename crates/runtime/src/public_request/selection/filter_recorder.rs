use std::collections::HashSet;

use any2api_domain::RoutingCredentialId;

use crate::{credential::CredentialFilterKind, routing::RouteCandidate};

#[derive(Default)]
pub(super) struct RequestFilterRecorder {
    seen: HashSet<(RoutingCredentialId, CredentialFilterKind)>,
}

impl RequestFilterRecorder {
    pub(super) fn record(&mut self, candidate: &RouteCandidate, kind: CredentialFilterKind) {
        if self.seen.insert((candidate.credential_id, kind)) {
            candidate.record_filter(kind);
        }
    }
}

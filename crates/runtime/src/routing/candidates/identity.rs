use std::collections::{HashMap, HashSet};

use any2api_domain::{
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, RouteTargetId, RoutingCredentialId,
};
use tokio::time::Instant;

use super::RouteCandidate;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct EgressPathIdentity {
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) endpoint_config_version: u64,
    pub(crate) proxy_id: ProxyProfileId,
    pub(crate) proxy_config_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct CredentialGenerationIdentity {
    pub(super) credential_id: RoutingCredentialId,
    pub(super) routing_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CandidateIdentity {
    pub(crate) target_id: RouteTargetId,
    pub(crate) operation: ProtocolOperation,
    pub(crate) credential_id: RoutingCredentialId,
    pub(crate) routing_generation: u64,
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) endpoint_config_version: u64,
    pub(crate) proxy_id: ProxyProfileId,
    pub(crate) proxy_config_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateFailureScope {
    ExactCandidate,
    Credential,
    CredentialModel,
    RouteOperation,
    EgressPath,
    Proxy,
    Endpoint,
}

type RouteOperationIdentity = (RouteTargetId, ProtocolOperation);

#[derive(Debug, Default)]
pub(crate) struct CandidateSelectionState {
    candidates: HashSet<CandidateIdentity>,
    credentials: HashSet<RoutingCredentialId>,
    credential_models: HashMap<CredentialGenerationIdentity, HashSet<String>>,
    route_operations: HashSet<RouteOperationIdentity>,
    endpoints: HashSet<ProviderEndpointId>,
    proxies: HashSet<ProxyProfileId>,
    egress_paths: HashSet<EgressPathIdentity>,
    attempted_candidates: HashSet<CandidateIdentity>,
    attempted_credentials: HashSet<CredentialGenerationIdentity>,
    attempted_egress_paths: HashSet<EgressPathIdentity>,
    candidate_not_before: HashMap<CandidateIdentity, Instant>,
    credential_not_before: HashMap<CredentialGenerationIdentity, Instant>,
    credential_model_not_before: HashMap<CredentialGenerationIdentity, HashMap<String, Instant>>,
    route_operation_not_before: HashMap<RouteOperationIdentity, Instant>,
    egress_path_not_before: HashMap<EgressPathIdentity, Instant>,
    proxy_not_before: HashMap<ProxyProfileId, Instant>,
    endpoint_not_before: HashMap<ProviderEndpointId, Instant>,
}

impl CandidateSelectionState {
    pub(crate) const RETRY_PREFERENCE_LEVELS: u8 = 5;

    pub(crate) fn allows(&self, candidate: &RouteCandidate) -> bool {
        !self.candidates.contains(&candidate.identity())
            && !self.credentials.contains(&candidate.credential_id)
            && !self.credential_model_excluded(candidate)
            && !self
                .route_operations
                .contains(&route_operation_identity(candidate))
            && !self.endpoints.contains(&candidate.endpoint_id)
            && !self.proxies.contains(&candidate.proxy_id)
            && !self
                .egress_paths
                .contains(&candidate.egress_path_identity())
    }

    pub(crate) fn retry_preference(&self, candidate: &RouteCandidate) -> u8 {
        if self.attempted_candidates.contains(&candidate.identity()) {
            return 4;
        }
        let new_credential = !self
            .attempted_credentials
            .contains(&candidate.credential_generation_identity());
        let new_egress = !self
            .attempted_egress_paths
            .contains(&candidate.egress_path_identity());
        match (new_credential, new_egress) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        }
    }

    pub(crate) fn note_failed(&mut self, candidate: &RouteCandidate) {
        self.attempted_candidates.insert(candidate.identity());
        self.attempted_credentials
            .insert(candidate.credential_generation_identity());
        self.attempted_egress_paths
            .insert(candidate.egress_path_identity());
    }

    pub(crate) fn exclude(&mut self, candidate: &RouteCandidate, scope: CandidateFailureScope) {
        match scope {
            CandidateFailureScope::ExactCandidate => {
                self.candidates.insert(candidate.identity());
            }
            CandidateFailureScope::Credential => {
                self.credentials.insert(candidate.credential_id);
            }
            CandidateFailureScope::CredentialModel => {
                self.credential_models
                    .entry(candidate.credential_generation_identity())
                    .or_default()
                    .insert(candidate.upstream_model.clone());
            }
            CandidateFailureScope::RouteOperation => {
                self.route_operations
                    .insert(route_operation_identity(candidate));
            }
            CandidateFailureScope::EgressPath => {
                self.egress_paths.insert(candidate.egress_path_identity());
            }
            CandidateFailureScope::Proxy => {
                self.proxies.insert(candidate.proxy_id);
            }
            CandidateFailureScope::Endpoint => {
                self.endpoints.insert(candidate.endpoint_id);
            }
        }
    }

    pub(crate) fn defer(
        &mut self,
        candidate: &RouteCandidate,
        scope: CandidateFailureScope,
        not_before: Instant,
    ) {
        match scope {
            CandidateFailureScope::ExactCandidate => {
                extend_deadline(
                    &mut self.candidate_not_before,
                    candidate.identity(),
                    not_before,
                );
            }
            CandidateFailureScope::Credential => extend_deadline(
                &mut self.credential_not_before,
                candidate.credential_generation_identity(),
                not_before,
            ),
            CandidateFailureScope::CredentialModel => {
                extend_deadline(
                    self.credential_model_not_before
                        .entry(candidate.credential_generation_identity())
                        .or_default(),
                    candidate.upstream_model.clone(),
                    not_before,
                );
            }
            CandidateFailureScope::RouteOperation => extend_deadline(
                &mut self.route_operation_not_before,
                route_operation_identity(candidate),
                not_before,
            ),
            CandidateFailureScope::EgressPath => extend_deadline(
                &mut self.egress_path_not_before,
                candidate.egress_path_identity(),
                not_before,
            ),
            CandidateFailureScope::Proxy => {
                extend_deadline(&mut self.proxy_not_before, candidate.proxy_id, not_before);
            }
            CandidateFailureScope::Endpoint => {
                extend_deadline(
                    &mut self.endpoint_not_before,
                    candidate.endpoint_id,
                    not_before,
                );
            }
        }
    }

    pub(crate) fn deferred_until(&self, candidate: &RouteCandidate) -> Option<Instant> {
        [
            self.candidate_not_before.get(&candidate.identity()),
            self.credential_not_before
                .get(&candidate.credential_generation_identity()),
            self.credential_model_not_before
                .get(&candidate.credential_generation_identity())
                .and_then(|models| models.get(&candidate.upstream_model)),
            self.route_operation_not_before
                .get(&route_operation_identity(candidate)),
            self.egress_path_not_before
                .get(&candidate.egress_path_identity()),
            self.proxy_not_before.get(&candidate.proxy_id),
            self.endpoint_not_before.get(&candidate.endpoint_id),
        ]
        .into_iter()
        .flatten()
        .copied()
        .max()
    }

    fn credential_model_excluded(&self, candidate: &RouteCandidate) -> bool {
        self.credential_models
            .get(&candidate.credential_generation_identity())
            .is_some_and(|models| models.contains(&candidate.upstream_model))
    }
}

const fn route_operation_identity(candidate: &RouteCandidate) -> RouteOperationIdentity {
    (candidate.target_id, candidate.operation)
}

fn extend_deadline<K: Eq + std::hash::Hash>(
    deadlines: &mut HashMap<K, Instant>,
    key: K,
    not_before: Instant,
) {
    deadlines
        .entry(key)
        .and_modify(|current| *current = (*current).max(not_before))
        .or_insert(not_before);
}

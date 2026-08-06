use std::collections::{HashMap, HashSet};

use any2api_domain::{
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, RouteTargetId, RoutingCredentialId,
};

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

#[derive(Debug, Default)]
pub(crate) struct CandidateExclusions {
    candidates: HashSet<CandidateIdentity>,
    credentials: HashSet<RoutingCredentialId>,
    credential_models: HashMap<CredentialGenerationIdentity, HashSet<String>>,
    targets: HashSet<RouteTargetId>,
    endpoints: HashSet<ProviderEndpointId>,
    proxies: HashSet<ProxyProfileId>,
    egress_paths: HashSet<EgressPathIdentity>,
    attempted_credentials: HashSet<CredentialGenerationIdentity>,
    attempted_egress_paths: HashSet<EgressPathIdentity>,
}

impl CandidateExclusions {
    pub(crate) const RETRY_PREFERENCE_LEVELS: u8 = 4;

    pub(crate) fn allows(&self, candidate: &RouteCandidate) -> bool {
        !self.candidates.contains(&candidate.identity())
            && !self.credentials.contains(&candidate.credential_id)
            && !self.credential_model_excluded(candidate)
            && !self.targets.contains(&candidate.target_id)
            && !self.endpoints.contains(&candidate.endpoint_id)
            && !self.proxies.contains(&candidate.proxy_id)
            && !self
                .egress_paths
                .contains(&candidate.egress_path_identity())
    }

    pub(crate) fn retry_preference(&self, candidate: &RouteCandidate) -> u8 {
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

    pub(crate) fn note_failed_candidate(&mut self, candidate: &RouteCandidate) {
        self.attempted_credentials
            .insert(candidate.credential_generation_identity());
        self.attempted_egress_paths
            .insert(candidate.egress_path_identity());
    }

    pub(crate) fn exclude_candidate(&mut self, candidate: &RouteCandidate) {
        self.candidates.insert(candidate.identity());
    }

    pub(crate) fn exclude_credential(&mut self, id: RoutingCredentialId) {
        self.credentials.insert(id);
    }

    pub(crate) fn exclude_credential_model(&mut self, candidate: &RouteCandidate) {
        self.credential_models
            .entry(candidate.credential_generation_identity())
            .or_default()
            .insert(candidate.upstream_model.clone());
    }

    pub(crate) fn exclude_target(&mut self, id: RouteTargetId) {
        self.targets.insert(id);
    }

    pub(crate) fn exclude_endpoint(&mut self, id: ProviderEndpointId) {
        self.endpoints.insert(id);
    }

    pub(crate) fn exclude_proxy(&mut self, id: ProxyProfileId) {
        self.proxies.insert(id);
    }

    pub(crate) fn exclude_egress_path(&mut self, path: EgressPathIdentity) {
        self.egress_paths.insert(path);
    }

    fn credential_model_excluded(&self, candidate: &RouteCandidate) -> bool {
        self.credential_models
            .get(&candidate.credential_generation_identity())
            .is_some_and(|models| models.contains(&candidate.upstream_model))
    }
}

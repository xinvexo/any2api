use std::sync::Arc;

use any2api_domain::{UpstreamErrorClassification, UpstreamErrorKind, UpstreamFailureAttribution};
use any2api_transport::api::TransportFailureScope;
use tokio::time::Instant;

use super::{endpoint::EndpointPermit, proxy::ProxyPermit};
use crate::{credential::CredentialGenerationRuntime, health::ReliabilityPolicy};

pub(crate) struct AttemptHealth {
    credential: Arc<CredentialGenerationRuntime>,
    model: String,
    endpoint: Option<EndpointPermit>,
    proxy: Option<ProxyPermit>,
    egress_path: Option<EndpointPermit>,
    candidate_path: Option<EndpointPermit>,
    policy: ReliabilityPolicy,
    started_at: Instant,
    completed: bool,
}

impl AttemptHealth {
    #[cfg(test)]
    pub(crate) fn new(
        credential: Arc<CredentialGenerationRuntime>,
        model: String,
        endpoint: Option<EndpointPermit>,
        proxy: Option<ProxyPermit>,
        policy: ReliabilityPolicy,
    ) -> Self {
        Self::new_with_paths(credential, model, endpoint, proxy, None, None, policy)
    }

    pub(crate) fn new_with_paths(
        credential: Arc<CredentialGenerationRuntime>,
        model: String,
        endpoint: Option<EndpointPermit>,
        proxy: Option<ProxyPermit>,
        egress_path: Option<EndpointPermit>,
        candidate_path: Option<EndpointPermit>,
        policy: ReliabilityPolicy,
    ) -> Self {
        Self {
            credential,
            model,
            endpoint,
            proxy,
            egress_path,
            candidate_path,
            policy,
            started_at: Instant::now(),
            completed: false,
        }
    }

    pub(crate) fn success(mut self) {
        self.credential.health().record_success();
        if let Some(endpoint) = self.endpoint.take() {
            endpoint.success(self.started_at);
        }
        if let Some(path) = self.egress_path.take() {
            path.success(self.started_at);
        }
        if let Some(candidate) = self.candidate_path.take() {
            candidate.success(self.started_at);
        }
        if let Some(proxy) = self.proxy.take() {
            proxy.success();
        }
        self.completed = true;
    }

    pub(crate) fn upstream_failure(mut self, classification: UpstreamErrorClassification) {
        let attribution = classification.attribution();
        self.credential
            .health()
            .record(&self.model, classification, &self.policy);
        match attribution {
            UpstreamFailureAttribution::Endpoint => {
                neutral(&mut self.egress_path);
                neutral(&mut self.candidate_path);
                failure(&mut self.endpoint, &self.policy);
            }
            UpstreamFailureAttribution::EgressPath => {
                neutral(&mut self.endpoint);
                neutral(&mut self.candidate_path);
                failure(&mut self.egress_path, &self.policy);
            }
            UpstreamFailureAttribution::Authentication
            | UpstreamFailureAttribution::Credential
            | UpstreamFailureAttribution::CredentialModel => {
                neutral(&mut self.endpoint);
                success(&mut self.egress_path, self.started_at);
                success(&mut self.candidate_path, self.started_at);
            }
            UpstreamFailureAttribution::RouteOperation => {
                neutral(&mut self.endpoint);
                success(&mut self.egress_path, self.started_at);
                failure(&mut self.candidate_path, &self.policy);
            }
            UpstreamFailureAttribution::Unattributed => {
                neutral(&mut self.endpoint);
                neutral(&mut self.egress_path);
                if classification.kind() != UpstreamErrorKind::InvalidRequest {
                    failure(&mut self.candidate_path, &self.policy);
                } else {
                    neutral(&mut self.candidate_path);
                }
            }
        }
        if let Some(proxy) = self.proxy.take() {
            proxy.success();
        }
        self.completed = true;
    }

    pub(crate) fn transport_failure(mut self, failure_scope: TransportFailureScope) {
        match failure_scope {
            TransportFailureScope::Endpoint => {
                neutral(&mut self.egress_path);
                neutral(&mut self.candidate_path);
                if let Some(proxy) = self.proxy.take() {
                    proxy.neutral();
                }
                if let Some(endpoint) = self.endpoint.take() {
                    endpoint.failure(&self.policy);
                }
            }
            TransportFailureScope::Proxy => {
                neutral(&mut self.endpoint);
                neutral(&mut self.egress_path);
                neutral(&mut self.candidate_path);
                if let Some(proxy) = self.proxy.take() {
                    proxy.failure(&self.policy);
                }
            }
            TransportFailureScope::EgressPath => {
                neutral(&mut self.endpoint);
                neutral_proxy(&mut self.proxy);
                failure(&mut self.egress_path, &self.policy);
                failure(&mut self.candidate_path, &self.policy);
            }
            TransportFailureScope::Unattributed => {
                neutral(&mut self.endpoint);
                neutral_proxy(&mut self.proxy);
                neutral(&mut self.egress_path);
                failure(&mut self.candidate_path, &self.policy);
            }
        }
        self.completed = true;
    }
}

impl Drop for AttemptHealth {
    fn drop(&mut self) {
        if !self.completed {
            if let Some(endpoint) = self.endpoint.take() {
                endpoint.neutral();
            }
            if let Some(path) = self.egress_path.take() {
                path.neutral();
            }
            if let Some(candidate) = self.candidate_path.take() {
                candidate.neutral();
            }
            self.proxy.take();
        }
    }
}

fn neutral(permit: &mut Option<EndpointPermit>) {
    if let Some(permit) = permit.take() {
        permit.neutral();
    }
}

fn success(permit: &mut Option<EndpointPermit>, started_at: Instant) {
    if let Some(permit) = permit.take() {
        permit.success(started_at);
    }
}

fn failure(permit: &mut Option<EndpointPermit>, policy: &ReliabilityPolicy) {
    if let Some(permit) = permit.take() {
        permit.failure(policy);
    }
}

fn neutral_proxy(permit: &mut Option<ProxyPermit>) {
    if let Some(permit) = permit.take() {
        permit.neutral();
    }
}

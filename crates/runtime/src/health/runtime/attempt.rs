use std::sync::Arc;

use any2api_domain::{UpstreamErrorClassification, UpstreamErrorKind};
use any2api_transport::api::TransportFailureScope;
use tokio::time::Instant;

use super::{endpoint::EndpointPermit, proxy::ProxyPermit};
use crate::{credential_runtime::CredentialGenerationRuntime, health::ReliabilityPolicy};

pub(crate) struct AttemptHealth {
    credential: Arc<CredentialGenerationRuntime>,
    model: String,
    endpoint: Option<EndpointPermit>,
    proxy: Option<ProxyPermit>,
    policy: ReliabilityPolicy,
    started_at: Instant,
    completed: bool,
}

impl AttemptHealth {
    pub(crate) fn new(
        credential: Arc<CredentialGenerationRuntime>,
        model: String,
        endpoint: Option<EndpointPermit>,
        proxy: Option<ProxyPermit>,
        policy: ReliabilityPolicy,
    ) -> Self {
        Self {
            credential,
            model,
            endpoint,
            proxy,
            policy,
            started_at: Instant::now(),
            completed: false,
        }
    }

    pub(crate) fn success(mut self) {
        if let Some(endpoint) = self.endpoint.take() {
            endpoint.success(self.started_at);
        }
        if let Some(proxy) = self.proxy.take() {
            proxy.success();
        }
        self.completed = true;
    }

    pub(crate) fn upstream_failure(mut self, classification: UpstreamErrorClassification) {
        self.credential
            .health()
            .record(&self.model, classification, &self.policy);
        if classification.kind() == UpstreamErrorKind::Transient {
            if let Some(endpoint) = self.endpoint.take() {
                endpoint.failure(&self.policy);
            }
        } else if let Some(endpoint) = self.endpoint.take() {
            endpoint.neutral();
        }
        if let Some(proxy) = self.proxy.take() {
            proxy.success();
        }
        self.completed = true;
    }

    pub(crate) fn transport_failure(mut self, failure_scope: TransportFailureScope) {
        match failure_scope {
            TransportFailureScope::Endpoint => {
                if let Some(proxy) = self.proxy.take() {
                    proxy.neutral();
                }
                if let Some(endpoint) = self.endpoint.take() {
                    endpoint.failure(&self.policy);
                }
            }
            TransportFailureScope::Proxy => {
                if let Some(endpoint) = self.endpoint.take() {
                    endpoint.neutral();
                }
                if let Some(proxy) = self.proxy.take() {
                    proxy.failure(&self.policy);
                }
            }
            TransportFailureScope::Unattributed => {
                if let Some(endpoint) = self.endpoint.take() {
                    endpoint.neutral();
                }
                if let Some(proxy) = self.proxy.take() {
                    proxy.neutral();
                }
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
            self.proxy.take();
        }
    }
}

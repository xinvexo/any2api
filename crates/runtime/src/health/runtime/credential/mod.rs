mod authentication;
mod routing;

use std::sync::Arc;

use any2api_domain::{UpstreamErrorClassification, UpstreamErrorKind};

use self::{
    authentication::AuthenticationHealthRuntime,
    routing::{CredentialQuotaExhaustion, RoutingCredentialHealthRuntime},
};
use super::error::HealthAcquireError;
use crate::{health::ReliabilityPolicy, routing::SchedulerEpoch};

#[derive(Debug)]
pub(crate) struct CredentialHealthRuntime {
    authentication: AuthenticationHealthRuntime,
    routing: Arc<RoutingCredentialHealthRuntime>,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

impl CredentialHealthRuntime {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        Arc::new(Self {
            authentication: AuthenticationHealthRuntime::new(Arc::clone(&scheduler_epoch)),
            routing: RoutingCredentialHealthRuntime::new(Arc::clone(&scheduler_epoch)),
            scheduler_epoch,
        })
    }

    pub(crate) fn with_fresh_authentication(&self) -> Arc<Self> {
        Arc::new(Self {
            authentication: AuthenticationHealthRuntime::new(Arc::clone(&self.scheduler_epoch)),
            routing: Arc::clone(&self.routing),
            scheduler_epoch: Arc::clone(&self.scheduler_epoch),
        })
    }

    pub(crate) fn availability(&self, model: &str) -> Result<(), HealthAcquireError> {
        self.authentication.availability()?;
        self.routing.availability(model)
    }

    pub(crate) fn clear_auth_error(&self) -> bool {
        self.authentication.clear_auth_error()
    }

    pub(crate) fn record_authentication_failure(&self) -> bool {
        self.authentication.record_authentication_failure()
    }

    pub(crate) fn clear_temporary_cooldowns(&self) -> bool {
        self.routing.clear_temporary_cooldowns()
    }

    #[cfg(test)]
    pub(crate) fn has_auth_error(&self) -> bool {
        self.authentication.has_auth_error()
    }

    pub(crate) fn quota_exhaustion(&self) -> Option<CredentialQuotaExhaustion> {
        self.routing.quota_exhaustion()
    }

    pub(crate) fn clear_quota_exhaustion(&self) -> bool {
        self.routing.clear_quota_exhaustion()
    }

    pub(crate) fn record_quota_exhaustion(
        &self,
        delay: std::time::Duration,
        used: Option<u64>,
        limit: Option<u64>,
    ) {
        self.routing.record_quota_exhaustion(delay, used, limit);
    }

    pub(crate) fn record_success(&self) {
        self.routing.record_success();
    }

    pub(crate) fn record(
        &self,
        model: &str,
        classification: UpstreamErrorClassification,
        policy: &ReliabilityPolicy,
    ) {
        if classification.kind() == UpstreamErrorKind::Authentication {
            self.authentication.record_authentication_failure();
        } else {
            self.routing.record(model, classification, policy);
        }
    }

    #[cfg(test)]
    pub(crate) fn model_cooldown_count(&self) -> usize {
        self.routing.model_cooldown_count()
    }
}

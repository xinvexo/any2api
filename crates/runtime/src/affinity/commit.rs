use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use any2api_domain::ProtocolOperation;
use any2api_protocol::api::ProtocolContinuationState;

use super::{AffinityError, AffinityRegistry, AffinityTarget, ContinuationLease};

#[derive(Clone, Debug)]
pub(crate) struct ContinuationBindingCommitter {
    operation: ProtocolOperation,
    registry: Arc<AffinityRegistry>,
    target: AffinityTarget,
    ttl: Duration,
}

impl ContinuationBindingCommitter {
    pub(crate) fn new(
        operation: ProtocolOperation,
        registry: Arc<AffinityRegistry>,
        target: AffinityTarget,
        ttl: Duration,
    ) -> Self {
        Self {
            operation,
            registry,
            target,
            ttl,
        }
    }

    pub(crate) const fn operation(&self) -> ProtocolOperation {
        self.operation
    }

    pub(crate) fn bind_ready(
        &self,
        raw: &str,
        state: Option<ProtocolContinuationState>,
    ) -> Result<(), AffinityError> {
        self.registry
            .bind_ready_continuation(raw, self.target.clone(), state, self.ttl)
    }

    pub(crate) fn bind_ready_before(
        &self,
        raw: &str,
        state: Option<ProtocolContinuationState>,
        deadline: Instant,
    ) -> Result<(), AffinityError> {
        self.registry.bind_ready_continuation_before(
            raw,
            self.target.clone(),
            state,
            self.ttl,
            deadline,
        )
    }

    pub(crate) fn begin_pending(&self, raw: &str) -> Result<ContinuationLease, AffinityError> {
        self.registry
            .begin_pending_continuation(raw, self.target.clone(), self.ttl)
    }

    pub(crate) fn begin_pending_before(
        &self,
        raw: &str,
        deadline: Instant,
    ) -> Result<ContinuationLease, AffinityError> {
        self.registry.begin_pending_continuation_before(
            raw,
            self.target.clone(),
            self.ttl,
            deadline,
        )
    }
}

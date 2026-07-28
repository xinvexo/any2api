use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use any2api_domain::ProtocolOperation;

use super::{AffinityError, AffinityRegistry, AffinityTarget};

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

    pub(crate) fn bind(&self, raw: &str) -> Result<(), AffinityError> {
        self.registry
            .bind_continuation(raw, self.target.clone(), self.ttl)
    }

    pub(crate) fn bind_before(&self, raw: &str, deadline: Instant) -> Result<(), AffinityError> {
        self.registry
            .bind_continuation_before(raw, self.target.clone(), self.ttl, deadline)
    }
}

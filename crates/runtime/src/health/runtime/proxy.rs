use std::sync::Arc;

use super::error::HealthAcquireError;
use crate::{
    health::{
        ReliabilityPolicy,
        circuit::{CircuitPermit, CircuitRuntime},
    },
    scheduler_epoch::SchedulerEpoch,
};

#[derive(Debug)]
pub(crate) struct ProxyHealthRuntime {
    circuit: Arc<CircuitRuntime>,
}

impl ProxyHealthRuntime {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        Arc::new(Self {
            circuit: CircuitRuntime::new(scheduler_epoch),
        })
    }

    pub(crate) fn try_acquire(
        self: &Arc<Self>,
        policy: &ReliabilityPolicy,
    ) -> Result<ProxyPermit, HealthAcquireError> {
        self.circuit
            .try_acquire(policy.half_open_max_probes)
            .map(|permit| ProxyPermit {
                _runtime: Arc::clone(self),
                permit: Some(permit),
            })
            .map_err(HealthAcquireError::Temporary)
    }

    pub(crate) fn availability(
        &self,
        policy: &ReliabilityPolicy,
    ) -> Result<(), HealthAcquireError> {
        self.circuit
            .availability(policy.half_open_max_probes)
            .map_err(HealthAcquireError::Temporary)
    }
}

pub(crate) struct ProxyPermit {
    _runtime: Arc<ProxyHealthRuntime>,
    permit: Option<CircuitPermit>,
}

impl ProxyPermit {
    pub(crate) fn success(mut self) {
        if let Some(permit) = self.permit.take() {
            permit.success();
        }
    }

    pub(crate) fn failure(mut self, policy: &ReliabilityPolicy) {
        if let Some(permit) = self.permit.take() {
            let _ = permit.failure(
                policy.proxy_failure_threshold,
                policy.proxy_failure_window,
                policy.proxy_open_duration,
            );
        }
    }

    pub(crate) fn neutral(mut self) {
        self.permit.take();
    }
}

impl Drop for ProxyPermit {
    fn drop(&mut self) {
        self.permit.take();
    }
}

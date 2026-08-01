use std::sync::{Arc, Mutex};

use tokio::time::Instant;

use super::{
    error::HealthAcquireError,
    time::{deadline, max_deadline},
};
use crate::{
    health::{
        ReliabilityPolicy,
        circuit::{CircuitPermit, CircuitRuntime},
    },
    routing::{SchedulerEpoch, SchedulerWakeSlot},
};

#[derive(Debug)]
pub(crate) struct EndpointHealthRuntime {
    circuit: Arc<CircuitRuntime>,
    transient_until: Mutex<Option<Instant>>,
    transient_wake: SchedulerWakeSlot,
}

impl EndpointHealthRuntime {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        let transient_wake = scheduler_epoch.wake_slot();
        Arc::new(Self {
            circuit: CircuitRuntime::new(Arc::clone(&scheduler_epoch)),
            transient_until: Mutex::new(None),
            transient_wake,
        })
    }

    pub(crate) fn try_acquire(
        self: &Arc<Self>,
        policy: &ReliabilityPolicy,
    ) -> Result<EndpointPermit, HealthAcquireError> {
        self.check_transient()?;
        self.circuit
            .try_acquire(policy.half_open_max_probes)
            .map(|permit| EndpointPermit {
                runtime: Arc::clone(self),
                permit: Some(permit),
            })
            .map_err(HealthAcquireError::Temporary)
    }

    pub(crate) fn availability(
        &self,
        policy: &ReliabilityPolicy,
    ) -> Result<(), HealthAcquireError> {
        self.check_transient()?;
        self.circuit
            .availability(policy.half_open_max_probes)
            .map_err(HealthAcquireError::Temporary)
    }

    fn check_transient(&self) -> Result<(), HealthAcquireError> {
        let now = Instant::now();
        let current = *self
            .transient_until
            .lock()
            .expect("endpoint health lock poisoned");
        match current {
            Some(until) if now < until => Err(HealthAcquireError::Temporary(until)),
            _ => Ok(()),
        }
    }

    fn transient(&self, policy: &ReliabilityPolicy, permit: Option<CircuitPermit>) {
        let until = deadline(Instant::now(), policy.transient_endpoint);
        let mut current = self
            .transient_until
            .lock()
            .expect("endpoint health lock poisoned");
        *current = max_deadline(*current, Some(until));
        self.transient_wake
            .schedule(current.expect("transient deadline was just recorded"));
        drop(current);
        if let Some(permit) = permit {
            let _ = permit.failure(
                policy.endpoint_failure_threshold,
                policy.endpoint_failure_window,
                policy.endpoint_open_duration,
            );
        }
    }

    fn success(&self, started_at: Instant, permit: Option<CircuitPermit>) {
        if let Some(permit) = permit {
            permit.success();
        }
        let mut current = self
            .transient_until
            .lock()
            .expect("endpoint health lock poisoned");
        if current.is_some_and(|until| until <= started_at) {
            *current = None;
            self.transient_wake.cancel();
        }
    }
}

pub(crate) struct EndpointPermit {
    runtime: Arc<EndpointHealthRuntime>,
    permit: Option<CircuitPermit>,
}

impl EndpointPermit {
    pub(crate) fn success(mut self, started_at: Instant) {
        self.runtime.success(started_at, self.permit.take());
    }

    pub(crate) fn failure(mut self, policy: &ReliabilityPolicy) {
        self.runtime.transient(policy, self.permit.take());
    }

    pub(crate) fn neutral(mut self) {
        self.permit.take();
    }
}

impl Drop for EndpointPermit {
    fn drop(&mut self) {
        self.permit.take();
    }
}

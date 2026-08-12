use any2api_domain::RequestTelemetryPosition;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct RequestTelemetryCheckpoint {
    pub(crate) process_id: Uuid,
    pub(crate) enabled: bool,
    pub(crate) policy_generation: u64,
    pub(crate) queue_dropped_request_logs: u64,
    pub(crate) storage_failed_request_logs: u64,
    pub(crate) pruned_request_logs: u64,
}

impl RequestTelemetryCheckpoint {
    pub(crate) fn covers_interval_to(&self, current: &Self) -> bool {
        self.process_id == current.process_id
            && self.enabled
            && current.enabled
            && self.policy_generation == current.policy_generation
            && self.queue_dropped_request_logs == current.queue_dropped_request_logs
            && self.storage_failed_request_logs == current.storage_failed_request_logs
            && self.pruned_request_logs == current.pruned_request_logs
    }

    pub(crate) fn preserves_persisted_interval_to(&self, current: &Self) -> bool {
        self.process_id == current.process_id
            && self.enabled
            && current.enabled
            && self.pruned_request_logs == current.pruned_request_logs
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RequestTelemetryObservation {
    pub(crate) observed_at_ms: u64,
    pub(crate) position: RequestTelemetryPosition,
    pub(crate) checkpoint: RequestTelemetryCheckpoint,
}

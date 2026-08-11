use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct RequestTelemetryCheckpoint {
    pub(crate) process_id: Uuid,
    pub(crate) enabled: bool,
    pub(crate) coverage_generation: u64,
    pub(crate) queue_dropped_request_logs: u64,
    pub(crate) storage_failed_request_logs: u64,
    pub(crate) pruned_request_logs: u64,
}

impl RequestTelemetryCheckpoint {
    pub(crate) fn covers_interval_to(&self, current: &Self) -> bool {
        self.process_id == current.process_id
            && self.enabled
            && current.enabled
            && self.coverage_generation == current.coverage_generation
    }
}

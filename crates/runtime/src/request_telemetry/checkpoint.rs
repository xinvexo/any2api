use any2api_domain::RequestTelemetryPosition;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Loss accounting for one OAuth account's completed RequestLogs, captured at
/// a quota observation fence. Queue and storage losses are tracked per
/// account so one account's telemetry gap does not invalidate another
/// account's observation interval; losses that could not be attributed to an
/// account fail closed for every account. Prune is tracked as the highest
/// telemetry sequence deleted from this process's persisted rows, so cleanup
/// of history older than an interval's anchor never invalidates it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestTelemetryCheckpoint {
    pub(crate) process_id: Uuid,
    pub(crate) enabled: bool,
    pub(crate) policy_generation: u64,
    pub(crate) account_queue_dropped_request_logs: u64,
    pub(crate) account_storage_failed_request_logs: u64,
    pub(crate) unattributed_lost_request_logs: u64,
    pub(crate) pruned_through_sequence: u64,
}

impl RequestTelemetryCheckpoint {
    /// Whether every completed RequestLog for this account between the two
    /// checkpoints was recorded and is still persisted. `interval_start_sequence`
    /// is the interval anchor's position; pruned rows at or before it are
    /// outside the interval and harmless.
    pub(crate) fn covers_interval_to(&self, current: &Self, interval_start_sequence: u64) -> bool {
        self.process_id == current.process_id
            && self.enabled
            && current.enabled
            && self.policy_generation == current.policy_generation
            && self.account_queue_dropped_request_logs == current.account_queue_dropped_request_logs
            && self.account_storage_failed_request_logs
                == current.account_storage_failed_request_logs
            && self.unattributed_lost_request_logs == current.unattributed_lost_request_logs
            && current.pruned_through_sequence <= interval_start_sequence
    }

    /// Whether rows persisted before this checkpoint are still intact in
    /// `current`: only prune can remove them, and only prune that reached past
    /// the interval anchor matters. Queue and storage losses after the fence
    /// belong to the next interval.
    pub(crate) fn preserves_persisted_interval_to(
        &self,
        current: &Self,
        interval_start_sequence: u64,
    ) -> bool {
        self.process_id == current.process_id
            && self.enabled
            && current.enabled
            && current.pruned_through_sequence <= interval_start_sequence
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestTelemetryObservation {
    pub(crate) observed_at_ms: u64,
    pub(crate) position: RequestTelemetryPosition,
    pub(crate) checkpoint: RequestTelemetryCheckpoint,
}

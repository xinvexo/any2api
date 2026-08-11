use any2api_provider::api::OAuthQuotaWindow;

use super::state::{QuotaObservationAnchor, QuotaWindowKey, QuotaWindowState};
use crate::{
    oauth::quota::types::{OAuthQuotaIntervalDiagnostic, OAuthQuotaIntervalStatus},
    request_telemetry::RequestTelemetryCheckpoint,
};

const RESET_JITTER_PERCENT: f64 = 0.5;

pub(super) fn must_reset(
    state: &QuotaWindowState,
    window: &OAuthQuotaWindow,
    checkpoint: &RequestTelemetryCheckpoint,
    fetched_at_ms: u64,
) -> bool {
    state.baseline.reset_at != window.reset_at
        || state.baseline.used_percent - window.used_percent > RESET_JITTER_PERCENT
        || (state.baseline.checkpoint.process_id != checkpoint.process_id
            && window.reset_at.is_none())
        || (window.reset_at.is_none()
            && window.limit_window_seconds.is_some_and(|seconds| {
                fetched_at_ms.saturating_sub(state.epoch_started_at_ms)
                    >= seconds.saturating_mul(1_000)
            }))
}

pub(super) fn new_window(
    key: QuotaWindowKey,
    epoch: u64,
    window: &OAuthQuotaWindow,
    checkpoint: RequestTelemetryCheckpoint,
    fetched_at_ms: u64,
    status: OAuthQuotaIntervalStatus,
) -> QuotaWindowState {
    QuotaWindowState {
        key,
        epoch,
        epoch_started_at_ms: fetched_at_ms,
        baseline: anchor(window, checkpoint, fetched_at_ms),
        samples: Vec::new(),
        latest_interval: OAuthQuotaIntervalDiagnostic {
            status,
            started_at: None,
            ended_at: seconds(fetched_at_ms),
            delta_used_percent: None,
            local_cost_credits: None,
            unpriced_request_count: 0,
            queue_dropped_request_logs: 0,
            storage_failed_request_logs: 0,
            pruned_request_logs: 0,
        },
    }
}

pub(super) fn anchor(
    window: &OAuthQuotaWindow,
    checkpoint: RequestTelemetryCheckpoint,
    fetched_at_ms: u64,
) -> QuotaObservationAnchor {
    QuotaObservationAnchor {
        fetched_at_ms,
        used_percent: window.used_percent,
        reset_at: window.reset_at,
        checkpoint,
    }
}

fn seconds(milliseconds: u64) -> i64 {
    i64::try_from(milliseconds / 1_000).unwrap_or(i64::MAX)
}

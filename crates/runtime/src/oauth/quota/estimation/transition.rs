use any2api_provider::api::OAuthQuotaWindow;

use super::state::{QuotaObservationAnchor, QuotaWindowKey, QuotaWindowState};
use crate::{
    oauth::quota::types::{OAuthQuotaIntervalDiagnostic, OAuthQuotaIntervalStatus},
    request_telemetry::RequestTelemetryObservation,
};

const RESET_JITTER_PERCENT: f64 = 0.5;
const RESET_AT_JITTER_SECONDS: u64 = 60;

pub(super) fn must_reset(
    state: &QuotaWindowState,
    window: &OAuthQuotaWindow,
    observation: &RequestTelemetryObservation,
) -> bool {
    let previous = &state.last_observation;
    reset_identity_changed(previous.reset_at, window.reset_at)
        || previous.used_percent - window.used_percent > RESET_JITTER_PERCENT
        || (previous.telemetry.checkpoint.process_id != observation.checkpoint.process_id
            && window.reset_at.is_none())
        || (window.reset_at.is_none()
            && window.limit_window_seconds.is_some_and(|seconds| {
                observation
                    .observed_at_ms
                    .saturating_sub(state.epoch_started_at_ms)
                    >= seconds.saturating_mul(1_000)
            }))
}

fn reset_identity_changed(previous: Option<i64>, current: Option<i64>) -> bool {
    match (previous, current) {
        (Some(previous), Some(current)) => previous.abs_diff(current) > RESET_AT_JITTER_SECONDS,
        (None, None) => false,
        _ => true,
    }
}

pub(super) fn new_window(
    key: QuotaWindowKey,
    epoch: u64,
    window: &OAuthQuotaWindow,
    observation: RequestTelemetryObservation,
    status: OAuthQuotaIntervalStatus,
) -> QuotaWindowState {
    let observed_at_ms = observation.observed_at_ms;
    let anchor = anchor(window, observation);
    QuotaWindowState {
        key,
        epoch,
        epoch_started_at_ms: observed_at_ms,
        last_observation: anchor.clone(),
        sample_anchor: anchor,
        samples: Vec::new(),
        latest_interval: reset_diagnostic(status, observed_at_ms),
    }
}

/// A reset or natural rollover starts a new usage epoch: both observation
/// anchors move to the current reading and the open interval is discarded.
/// The accepted samples stay — capacity is a property of the subscription,
/// not of one 5-hour window, so learned knowledge survives as the prior and
/// keeps being refined by new samples.
pub(super) fn rollover_window(
    previous: QuotaWindowState,
    epoch: u64,
    window: &OAuthQuotaWindow,
    observation: RequestTelemetryObservation,
) -> QuotaWindowState {
    let observed_at_ms = observation.observed_at_ms;
    let anchor = anchor(window, observation);
    QuotaWindowState {
        key: previous.key,
        epoch,
        epoch_started_at_ms: observed_at_ms,
        last_observation: anchor.clone(),
        sample_anchor: anchor,
        samples: previous.samples,
        latest_interval: reset_diagnostic(OAuthQuotaIntervalStatus::ResetBoundary, observed_at_ms),
    }
}

pub(super) fn anchor(
    window: &OAuthQuotaWindow,
    telemetry: RequestTelemetryObservation,
) -> QuotaObservationAnchor {
    QuotaObservationAnchor {
        used_percent: window.used_percent,
        reset_at: window.reset_at,
        telemetry,
    }
}

fn reset_diagnostic(
    status: OAuthQuotaIntervalStatus,
    observed_at_ms: u64,
) -> OAuthQuotaIntervalDiagnostic {
    OAuthQuotaIntervalDiagnostic {
        status,
        started_at: None,
        ended_at: seconds(observed_at_ms),
        delta_used_percent: None,
        local_cost_credits: None,
        unpriced_request_count: 0,
        queue_dropped_request_logs: 0,
        storage_failed_request_logs: 0,
        interval_pruned: false,
    }
}

fn seconds(milliseconds: u64) -> i64 {
    i64::try_from(milliseconds / 1_000).unwrap_or(i64::MAX)
}

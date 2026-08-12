use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_provider::api::OAuthQuotaWindow;

use super::{
    OAuthQuotaEstimator, learning,
    state::{QuotaObservationAnchor, QuotaWindowState},
    transition,
};
use crate::{
    oauth::quota::types::{OAuthQuotaIntervalDiagnostic, OAuthQuotaIntervalStatus},
    request_telemetry::{
        RequestTelemetry, RequestTelemetryCheckpoint, RequestTelemetryObservation,
    },
};

const MIN_SAMPLE_DELTA_PERCENT: f64 = 0.5;

pub(super) async fn observe(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    window: &OAuthQuotaWindow,
    mut state: QuotaWindowState,
    expected_unit: QuotaCostUnit,
    observation: RequestTelemetryObservation,
    telemetry: Option<&RequestTelemetry>,
) -> QuotaWindowState {
    let sample_anchor = state.sample_anchor.clone();
    let started_at_ms = sample_anchor.telemetry.observed_at_ms;
    let ended_at_ms = observation.observed_at_ms;
    let delta = window.used_percent - sample_anchor.used_percent;
    let loss = checkpoint_loss(&sample_anchor.telemetry.checkpoint, &observation.checkpoint);
    let mut reanchor_sample = true;
    let diagnostic = if !delta.is_finite() {
        state.competing_samples.clear();
        diagnostic(
            OAuthQuotaIntervalStatus::Invalid,
            Some(started_at_ms),
            ended_at_ms,
            Some(delta),
            None,
            0,
            loss,
        )
    } else if !covers_interval(&sample_anchor, &observation) {
        state.competing_samples.clear();
        diagnostic(
            OAuthQuotaIntervalStatus::TelemetryIncomplete,
            Some(started_at_ms),
            ended_at_ms,
            Some(delta),
            None,
            0,
            loss,
        )
    } else if delta < MIN_SAMPLE_DELTA_PERCENT {
        reanchor_sample = false;
        diagnostic(
            OAuthQuotaIntervalStatus::NoChange,
            Some(started_at_ms),
            ended_at_ms,
            Some(delta),
            None,
            0,
            loss,
        )
    } else {
        estimate(
            estimator,
            id,
            &mut state,
            delta,
            expected_unit,
            &sample_anchor,
            &observation,
            telemetry,
        )
        .await
    };
    let current_anchor = transition::anchor(window, observation);
    if reanchor_sample {
        state.sample_anchor = current_anchor.clone();
    }
    state.last_observation = current_anchor;
    state.latest_interval = diagnostic;
    state
}

#[allow(clippy::too_many_arguments)]
async fn estimate(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    state: &mut QuotaWindowState,
    delta: f64,
    expected_unit: QuotaCostUnit,
    sample_anchor: &QuotaObservationAnchor,
    observation: &RequestTelemetryObservation,
    telemetry: Option<&RequestTelemetry>,
) -> OAuthQuotaIntervalDiagnostic {
    let usage_result = estimator
        .repository
        .oauth_quota_request_log_usage(id, sample_anchor.telemetry.position, observation.position)
        .await;
    let ending_checkpoint = match telemetry {
        Some(telemetry) => telemetry.quota_checkpoint().await,
        None => observation.checkpoint.clone(),
    };
    let fence_loss = checkpoint_loss(&sample_anchor.telemetry.checkpoint, &observation.checkpoint);
    if !observation
        .checkpoint
        .preserves_persisted_interval_to(&ending_checkpoint)
    {
        state.competing_samples.clear();
        return diagnostic(
            OAuthQuotaIntervalStatus::TelemetryIncomplete,
            Some(sample_anchor.telemetry.observed_at_ms),
            observation.observed_at_ms,
            Some(delta),
            None,
            0,
            checkpoint_loss(&sample_anchor.telemetry.checkpoint, &ending_checkpoint),
        );
    }
    let usage = match usage_result {
        Ok(usage) => usage,
        Err(error) => {
            state.competing_samples.clear();
            tracing::warn!(%error, %id, "quota interval RequestLog query failed");
            return diagnostic(
                OAuthQuotaIntervalStatus::Invalid,
                Some(sample_anchor.telemetry.observed_at_ms),
                observation.observed_at_ms,
                Some(delta),
                None,
                0,
                fence_loss,
            );
        }
    };
    let learned = learning::apply_usage(
        state,
        delta,
        expected_unit,
        usage,
        observation.observed_at_ms,
    );
    diagnostic(
        learned.status,
        Some(sample_anchor.telemetry.observed_at_ms),
        observation.observed_at_ms,
        Some(delta),
        Some(learned.local_cost_credits),
        learned.unpriced_request_count,
        fence_loss,
    )
}

fn covers_interval(start: &QuotaObservationAnchor, current: &RequestTelemetryObservation) -> bool {
    start
        .telemetry
        .checkpoint
        .covers_interval_to(&current.checkpoint)
        && start.telemetry.position.process_id == current.position.process_id
        && start.telemetry.position.sequence <= current.position.sequence
}

#[derive(Clone, Copy, Default)]
struct CheckpointLoss {
    queue_dropped: u64,
    storage_failed: u64,
    pruned: u64,
}

fn checkpoint_loss(
    previous: &RequestTelemetryCheckpoint,
    current: &RequestTelemetryCheckpoint,
) -> CheckpointLoss {
    if previous.process_id != current.process_id {
        return CheckpointLoss {
            queue_dropped: current.queue_dropped_request_logs,
            storage_failed: current.storage_failed_request_logs,
            pruned: current.pruned_request_logs,
        };
    }
    CheckpointLoss {
        queue_dropped: current
            .queue_dropped_request_logs
            .saturating_sub(previous.queue_dropped_request_logs),
        storage_failed: current
            .storage_failed_request_logs
            .saturating_sub(previous.storage_failed_request_logs),
        pruned: current
            .pruned_request_logs
            .saturating_sub(previous.pruned_request_logs),
    }
}

#[allow(clippy::too_many_arguments)]
fn diagnostic(
    status: OAuthQuotaIntervalStatus,
    started_at_ms: Option<u64>,
    ended_at_ms: u64,
    delta_used_percent: Option<f64>,
    local_cost_credits: Option<f64>,
    unpriced_request_count: u64,
    loss: CheckpointLoss,
) -> OAuthQuotaIntervalDiagnostic {
    let started_at = started_at_ms.map(seconds);
    let ended_at = seconds(ended_at_ms).max(started_at.unwrap_or_default());
    OAuthQuotaIntervalDiagnostic {
        status,
        started_at,
        ended_at,
        delta_used_percent,
        local_cost_credits,
        unpriced_request_count,
        queue_dropped_request_logs: loss.queue_dropped,
        storage_failed_request_logs: loss.storage_failed,
        pruned_request_logs: loss.pruned,
    }
}

fn seconds(milliseconds: u64) -> i64 {
    i64::try_from(milliseconds / 1_000).unwrap_or(i64::MAX)
}

use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_provider::api::OAuthQuotaWindow;
use any2api_storage::api::OAuthQuotaRequestLogUsage;

use super::{
    OAuthQuotaEstimator,
    robust::{self, CandidateDecision},
    state::{QuotaCapacitySample, QuotaWindowState},
    transition,
};
use crate::{
    oauth::quota::types::{OAuthQuotaIntervalDiagnostic, OAuthQuotaIntervalStatus},
    request_telemetry::{RequestTelemetry, RequestTelemetryCheckpoint},
};

const MIN_SAMPLE_DELTA_PERCENT: f64 = 0.5;
const MAX_SAMPLES_PER_EPOCH: usize = 9;
const NANOS_PER_CREDIT: f64 = 1_000_000_000.0;

#[allow(clippy::too_many_arguments)]
pub(super) async fn observe(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    window: &OAuthQuotaWindow,
    mut state: QuotaWindowState,
    expected_unit: QuotaCostUnit,
    checkpoint: RequestTelemetryCheckpoint,
    fetched_at_ms: u64,
    telemetry: Option<&RequestTelemetry>,
) -> QuotaWindowState {
    let started_at_ms = state.baseline.fetched_at_ms;
    let delta = window.used_percent - state.baseline.used_percent;
    let mut ending_checkpoint = checkpoint.clone();
    let mut loss = checkpoint_loss(&state.baseline.checkpoint, &checkpoint);
    let mut diagnostic = diagnostic(
        OAuthQuotaIntervalStatus::NoChange,
        Some(started_at_ms),
        fetched_at_ms,
        Some(delta),
        None,
        0,
        loss,
    );
    if fetched_at_ms <= started_at_ms || !delta.is_finite() {
        diagnostic.status = OAuthQuotaIntervalStatus::Invalid;
    } else if !state.baseline.checkpoint.covers_interval_to(&checkpoint) {
        diagnostic.status = OAuthQuotaIntervalStatus::TelemetryIncomplete;
    } else if delta < MIN_SAMPLE_DELTA_PERCENT {
        diagnostic.status = OAuthQuotaIntervalStatus::NoChange;
    } else {
        let interval = estimate(
            estimator,
            id,
            &state,
            delta,
            expected_unit,
            started_at_ms,
            fetched_at_ms,
            loss,
            &checkpoint,
            telemetry,
        )
        .await;
        diagnostic = interval.diagnostic;
        ending_checkpoint = interval.checkpoint;
        loss = checkpoint_loss(&state.baseline.checkpoint, &ending_checkpoint);
        apply_loss(&mut diagnostic, loss);
        if let Some((capacity_credits, rate_cards)) = interval.sample {
            state.samples.push(QuotaCapacitySample {
                capacity_credits,
                observed_at_ms: fetched_at_ms,
                rate_cards,
            });
            if state.samples.len() > MAX_SAMPLES_PER_EPOCH {
                state.samples.remove(0);
            }
        }
    }
    state.baseline = transition::anchor(window, ending_checkpoint, fetched_at_ms);
    state.latest_interval = diagnostic;
    state
}

#[allow(clippy::too_many_arguments)]
async fn estimate(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    state: &QuotaWindowState,
    delta: f64,
    expected_unit: QuotaCostUnit,
    started_at_ms: u64,
    fetched_at_ms: u64,
    loss: CheckpointLoss,
    checkpoint: &RequestTelemetryCheckpoint,
    telemetry: Option<&RequestTelemetry>,
) -> IntervalEstimation {
    let usage_result = estimator
        .repository
        .oauth_quota_request_log_usage(id, started_at_ms, fetched_at_ms)
        .await;
    let ending_checkpoint = match telemetry {
        Some(telemetry) => telemetry.quota_checkpoint().await,
        None => checkpoint.clone(),
    };
    if !checkpoint.covers_interval_to(&ending_checkpoint) {
        return rejected(
            OAuthQuotaIntervalStatus::TelemetryIncomplete,
            delta,
            started_at_ms,
            fetched_at_ms,
            loss,
            ending_checkpoint,
        );
    }
    let usage = match usage_result {
        Ok(usage) => usage,
        Err(error) => {
            tracing::warn!(%error, %id, "quota interval RequestLog query failed");
            return rejected(
                OAuthQuotaIntervalStatus::Invalid,
                delta,
                started_at_ms,
                fetched_at_ms,
                loss,
                ending_checkpoint,
            );
        }
    };
    classify_usage(
        state,
        delta,
        expected_unit,
        usage,
        started_at_ms,
        fetched_at_ms,
        loss,
        ending_checkpoint,
    )
}

#[allow(clippy::too_many_arguments)]
fn classify_usage(
    state: &QuotaWindowState,
    delta: f64,
    expected_unit: QuotaCostUnit,
    usage: OAuthQuotaRequestLogUsage,
    started_at_ms: u64,
    fetched_at_ms: u64,
    loss: CheckpointLoss,
    ending_checkpoint: RequestTelemetryCheckpoint,
) -> IntervalEstimation {
    let cost = usage.total_cost_nanos as f64 / NANOS_PER_CREDIT;
    let unpriced = usage.unpriced_request_count;
    let rate_cards = usage.rate_cards;
    let base = |status, sample| IntervalEstimation {
        diagnostic: diagnostic(
            status,
            Some(started_at_ms),
            fetched_at_ms,
            Some(delta),
            Some(cost),
            unpriced,
            loss,
        ),
        sample,
        checkpoint: ending_checkpoint.clone(),
    };
    if unpriced > 0 {
        return base(OAuthQuotaIntervalStatus::UnpricedUsage, None);
    }
    if usage.unit != Some(expected_unit) || usage.priced_request_count == 0 || cost <= 0.0 {
        return base(OAuthQuotaIntervalStatus::ExternalUsageSuspected, None);
    }
    let candidate = cost * 100.0 / delta;
    if !candidate.is_finite() || candidate <= 0.0 {
        return base(OAuthQuotaIntervalStatus::Invalid, None);
    }
    match robust::classify_candidate(&state.samples, candidate) {
        CandidateDecision::Accept => base(
            OAuthQuotaIntervalStatus::ValidSample,
            Some((candidate, rate_cards)),
        ),
        CandidateDecision::ExternalUsage => {
            base(OAuthQuotaIntervalStatus::ExternalUsageSuspected, None)
        }
        CandidateDecision::Outlier => base(OAuthQuotaIntervalStatus::OutlierRejected, None),
    }
}

fn rejected(
    status: OAuthQuotaIntervalStatus,
    delta: f64,
    started_at_ms: u64,
    fetched_at_ms: u64,
    loss: CheckpointLoss,
    checkpoint: RequestTelemetryCheckpoint,
) -> IntervalEstimation {
    IntervalEstimation {
        diagnostic: diagnostic(
            status,
            Some(started_at_ms),
            fetched_at_ms,
            Some(delta),
            None,
            0,
            loss,
        ),
        sample: None,
        checkpoint,
    }
}

struct IntervalEstimation {
    diagnostic: OAuthQuotaIntervalDiagnostic,
    sample: Option<(f64, Vec<String>)>,
    checkpoint: RequestTelemetryCheckpoint,
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

fn apply_loss(diagnostic: &mut OAuthQuotaIntervalDiagnostic, loss: CheckpointLoss) {
    diagnostic.queue_dropped_request_logs = loss.queue_dropped;
    diagnostic.storage_failed_request_logs = loss.storage_failed;
    diagnostic.pruned_request_logs = loss.pruned;
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
    OAuthQuotaIntervalDiagnostic {
        status,
        started_at: started_at_ms.map(seconds),
        ended_at: seconds(ended_at_ms),
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

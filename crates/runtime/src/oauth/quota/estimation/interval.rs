use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_provider::api::OAuthQuotaWindow;
use any2api_storage::api::OAuthQuotaRequestLogUsage;

use super::{
    OAuthQuotaEstimator,
    state::{MAX_ACCEPTED_SAMPLES, QuotaCapacitySample, QuotaObservationAnchor, QuotaWindowState},
    transition,
};
use crate::{
    oauth::quota::types::{OAuthQuotaIntervalDiagnostic, OAuthQuotaIntervalStatus},
    request_telemetry::{
        RequestTelemetry, RequestTelemetryCheckpoint, RequestTelemetryObservation,
    },
};

pub(super) const NANOS_PER_CREDIT: f64 = 1_000_000_000.0;

/// Minimum percent delta before the interval RequestLog query runs at all;
/// below it the observation only advances the last observation.
const PROBE_DELTA_PERCENT: f64 = 0.5;

/// Minimum accumulated Δused% before a clean interval mints a capacity
/// sample. Official percent quantization and accounting-timing skew are
/// absolute errors on the delta, so the relative error of a sample shrinks
/// with its denominator: at a worst-case ±0.5 endpoint quantization, a 5%
/// delta bounds the sample error near ±10% while 3% admits ±17%. The looser
/// bootstrap threshold buys an early learning-grade figure on a fresh window
/// and is quickly outweighed once ≥5% samples exist.
pub(super) const BOOTSTRAP_MINT_DELTA_PERCENT: f64 = 3.0;
pub(super) const STANDARD_MINT_DELTA_PERCENT: f64 = 5.0;

pub(super) fn mint_delta_percent(state: &QuotaWindowState) -> f64 {
    if state.samples.is_empty() {
        BOOTSTRAP_MINT_DELTA_PERCENT
    } else {
        STANDARD_MINT_DELTA_PERCENT
    }
}

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
    let loss = checkpoint_loss(&sample_anchor, &observation.checkpoint);
    let mut reanchor_sample = true;
    let diagnostic = if !delta.is_finite() {
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
        diagnostic(
            OAuthQuotaIntervalStatus::TelemetryIncomplete,
            Some(started_at_ms),
            ended_at_ms,
            Some(delta),
            None,
            0,
            loss,
        )
    } else if delta < PROBE_DELTA_PERCENT {
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
        let probed = probe(
            estimator,
            id,
            &mut state,
            delta,
            expected_unit,
            &sample_anchor,
            &observation,
            telemetry,
        )
        .await;
        reanchor_sample = probed.reanchor;
        probed.diagnostic
    };
    let current_anchor = transition::anchor(window, observation);
    if reanchor_sample {
        state.sample_anchor = current_anchor.clone();
    }
    state.last_observation = current_anchor;
    state.latest_interval = diagnostic;
    state
}

struct ProbeOutcome {
    diagnostic: OAuthQuotaIntervalDiagnostic,
    reanchor: bool,
}

/// Runs the fence-checked interval query once the accumulated delta crossed
/// the probe threshold, then either keeps accumulating, mints a capacity
/// sample, or discards the broken interval by re-anchoring. Incomplete
/// intervals are never repaired or partially harvested: a fresh anchor costs
/// one interval, a contaminated sample poisons the estimate.
#[allow(clippy::too_many_arguments)]
async fn probe(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    state: &mut QuotaWindowState,
    delta: f64,
    expected_unit: QuotaCostUnit,
    sample_anchor: &QuotaObservationAnchor,
    observation: &RequestTelemetryObservation,
    telemetry: Option<&RequestTelemetry>,
) -> ProbeOutcome {
    let started_at_ms = sample_anchor.telemetry.observed_at_ms;
    let ended_at_ms = observation.observed_at_ms;
    let usage_result = estimator
        .repository
        .oauth_quota_request_log_usage(id, sample_anchor.telemetry.position, observation.position)
        .await;
    let ending_checkpoint = match telemetry {
        Some(telemetry) => telemetry.quota_checkpoint(id).await,
        None => observation.checkpoint.clone(),
    };
    let fence_loss = checkpoint_loss(sample_anchor, &observation.checkpoint);
    if !observation.checkpoint.preserves_persisted_interval_to(
        &ending_checkpoint,
        sample_anchor.telemetry.position.sequence,
    ) {
        return reanchored(diagnostic(
            OAuthQuotaIntervalStatus::TelemetryIncomplete,
            Some(started_at_ms),
            ended_at_ms,
            Some(delta),
            None,
            0,
            checkpoint_loss(sample_anchor, &ending_checkpoint),
        ));
    }
    let usage = match usage_result {
        Ok(usage) => usage,
        Err(error) => {
            tracing::warn!(%error, %id, "quota interval RequestLog query failed");
            return reanchored(diagnostic(
                OAuthQuotaIntervalStatus::Invalid,
                Some(started_at_ms),
                ended_at_ms,
                Some(delta),
                None,
                0,
                fence_loss,
            ));
        }
    };
    let cost_credits = usage.total_cost_nanos as f64 / NANOS_PER_CREDIT;
    if usage.unpriced_request_count > 0 {
        let unpriced = usage.unpriced_request_count;
        return reanchored(diagnostic(
            OAuthQuotaIntervalStatus::UnpricedUsage,
            Some(started_at_ms),
            ended_at_ms,
            Some(delta),
            Some(cost_credits),
            unpriced,
            fence_loss,
        ));
    }
    // No priced cost yet: the official percent moved for requests that are
    // still in flight locally (cost freezes at completion) or for usage the
    // upstream accounted late. All consumption flows through this process, so
    // the matching cost lands in this same interval — hold the anchor and let
    // the interval telescope instead of splitting cost from its percent.
    let accumulating = usage.priced_request_count == 0 || cost_credits <= 0.0;
    if !accumulating && usage.unit != Some(expected_unit) {
        return reanchored(diagnostic(
            OAuthQuotaIntervalStatus::Invalid,
            Some(started_at_ms),
            ended_at_ms,
            Some(delta),
            Some(cost_credits),
            0,
            fence_loss,
        ));
    }
    if accumulating || delta < mint_delta_percent(state) {
        return ProbeOutcome {
            diagnostic: diagnostic(
                OAuthQuotaIntervalStatus::Accumulating,
                Some(started_at_ms),
                ended_at_ms,
                Some(delta),
                Some(cost_credits),
                0,
                fence_loss,
            ),
            reanchor: false,
        };
    }
    let status = mint(state, delta, usage, ended_at_ms);
    reanchored(diagnostic(
        status,
        Some(started_at_ms),
        ended_at_ms,
        Some(delta),
        Some(cost_credits),
        0,
        fence_loss,
    ))
}

/// `capacity = local_cost × 100 / Δused%`: with no consumer besides this
/// process, a clean interval measures the true capacity up to bounded
/// endpoint noise, so every finite positive candidate is accepted as-is.
fn mint(
    state: &mut QuotaWindowState,
    delta_used_percent: f64,
    usage: OAuthQuotaRequestLogUsage,
    observed_at_ms: u64,
) -> OAuthQuotaIntervalStatus {
    let local_cost_credits = usage.total_cost_nanos as f64 / NANOS_PER_CREDIT;
    let capacity_credits = local_cost_credits * 100.0 / delta_used_percent;
    if !capacity_credits.is_finite() || capacity_credits <= 0.0 {
        return OAuthQuotaIntervalStatus::Invalid;
    }
    state.samples.push(QuotaCapacitySample {
        capacity_credits,
        delta_used_percent,
        local_cost_credits,
        observed_at_ms,
        epoch: state.epoch,
        rate_cards: usage.rate_cards,
    });
    if state.samples.len() > MAX_ACCEPTED_SAMPLES {
        state.samples.remove(0);
    }
    OAuthQuotaIntervalStatus::ValidSample
}

fn reanchored(diagnostic: OAuthQuotaIntervalDiagnostic) -> ProbeOutcome {
    ProbeOutcome {
        diagnostic,
        reanchor: true,
    }
}

fn covers_interval(start: &QuotaObservationAnchor, current: &RequestTelemetryObservation) -> bool {
    start
        .telemetry
        .checkpoint
        .covers_interval_to(&current.checkpoint, start.telemetry.position.sequence)
        && start.telemetry.position.process_id == current.position.process_id
        && start.telemetry.position.sequence <= current.position.sequence
}

#[derive(Clone, Copy, Default)]
struct CheckpointLoss {
    queue_dropped: u64,
    storage_failed: u64,
    interval_pruned: bool,
}

fn checkpoint_loss(
    anchor: &QuotaObservationAnchor,
    current: &RequestTelemetryCheckpoint,
) -> CheckpointLoss {
    let previous = &anchor.telemetry.checkpoint;
    if previous.process_id != current.process_id {
        return CheckpointLoss {
            queue_dropped: current
                .account_queue_dropped_request_logs
                .saturating_add(current.unattributed_lost_request_logs),
            storage_failed: current.account_storage_failed_request_logs,
            interval_pruned: false,
        };
    }
    CheckpointLoss {
        queue_dropped: current
            .account_queue_dropped_request_logs
            .saturating_sub(previous.account_queue_dropped_request_logs)
            .saturating_add(
                current
                    .unattributed_lost_request_logs
                    .saturating_sub(previous.unattributed_lost_request_logs),
            ),
        storage_failed: current
            .account_storage_failed_request_logs
            .saturating_sub(previous.account_storage_failed_request_logs),
        interval_pruned: current.pruned_through_sequence > anchor.telemetry.position.sequence,
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
        interval_pruned: loss.interval_pruned,
    }
}

fn seconds(milliseconds: u64) -> i64 {
    i64::try_from(milliseconds / 1_000).unwrap_or(i64::MAX)
}

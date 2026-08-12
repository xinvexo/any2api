use any2api_storage::api::OAuthQuotaRequestLogUsage;

use super::state::{MAX_ACCEPTED_SAMPLES, QuotaCapacitySample, QuotaWindowState};
use crate::oauth::quota::types::OAuthQuotaIntervalStatus;

pub(super) const NANOS_PER_CREDIT: f64 = 1_000_000_000.0;
/// Minimum accumulated percent delta before a clean segment mints a capacity
/// sample. Bootstrapping trades precision for an early learning estimate; the
/// standard threshold keeps the denominator large against official percent
/// quantization and upstream accounting skew.
pub(super) const BOOTSTRAP_MINT_DELTA_PERCENT: f64 = 1.5;
pub(super) const STANDARD_MINT_DELTA_PERCENT: f64 = 5.0;
pub(super) const SALVAGE_DELTA_PERCENT: f64 = 1.5;
/// Candidates are lower bounds of the true capacity: external usage inflates
/// the percent denominator without local cost, so contamination only pushes
/// candidates down, while upward noise is bounded by accounting skew. The
/// cluster therefore accepts within a fixed band, ignores lows, and only two
/// mutually consistent high candidates may revise it upward.
const CLUSTER_BAND_RATIO: f64 = 0.25;

pub(super) struct LearningResult {
    pub(super) status: OAuthQuotaIntervalStatus,
    pub(super) local_cost_credits: f64,
}

pub(super) fn mint_delta_percent(state: &QuotaWindowState) -> f64 {
    if state.samples.is_empty() {
        BOOTSTRAP_MINT_DELTA_PERCENT
    } else {
        STANDARD_MINT_DELTA_PERCENT
    }
}

/// Mints a capacity candidate from a clean interval that reached the mint
/// threshold; contamination checks already ran at the probe.
pub(super) fn mint(
    state: &mut QuotaWindowState,
    delta_used_percent: f64,
    usage: OAuthQuotaRequestLogUsage,
    observed_at_ms: u64,
) -> LearningResult {
    let cost = usage.total_cost_nanos as f64 / NANOS_PER_CREDIT;
    let candidate = cost * 100.0 / delta_used_percent;
    if !candidate.is_finite() || candidate <= 0.0 {
        state.pending_high = None;
        return LearningResult {
            status: OAuthQuotaIntervalStatus::Invalid,
            local_cost_credits: cost,
        };
    }
    let sample = QuotaCapacitySample {
        capacity_credits: candidate,
        observed_at_ms,
        epoch: state.epoch,
        rate_cards: usage.rate_cards,
    };
    LearningResult {
        status: classify_and_store(state, sample),
        local_cost_credits: cost,
    }
}

/// Mint a capacity sample from the fence-verified clean prefix of the current
/// segment before the interval is re-anchored by a gap, contamination or
/// reset. The prefix cost was validated at its own probe; later loss does not
/// retroactively poison it.
pub(super) fn salvage_segment(state: &mut QuotaWindowState) {
    let Some(segment) = state.segment.take() else {
        return;
    };
    let delta = segment.ended_used_percent - state.sample_anchor.used_percent;
    if delta < SALVAGE_DELTA_PERCENT {
        return;
    }
    let candidate = segment.cost_nanos as f64 / NANOS_PER_CREDIT * 100.0 / delta;
    if !candidate.is_finite() || candidate <= 0.0 {
        return;
    }
    classify_and_store(
        state,
        QuotaCapacitySample {
            capacity_credits: candidate,
            observed_at_ms: segment.ended_at_ms,
            epoch: state.epoch,
            rate_cards: segment.rate_cards,
        },
    );
}

/// Marks an interval whose official delta carried no explainable local cost;
/// the strongest external-usage signal, so it advances the low streak without
/// touching the cluster or the pending high candidate.
pub(super) fn record_costless_delta(state: &mut QuotaWindowState) {
    state.low_streak = state.low_streak.saturating_add(1);
}

fn classify_and_store(
    state: &mut QuotaWindowState,
    sample: QuotaCapacitySample,
) -> OAuthQuotaIntervalStatus {
    let Some(center) = super::robust::median(&state.samples) else {
        accept(state, sample);
        return OAuthQuotaIntervalStatus::ValidSample;
    };
    if sample.capacity_credits < center * (1.0 - CLUSTER_BAND_RATIO) {
        state.low_streak = state.low_streak.saturating_add(1);
        return OAuthQuotaIntervalStatus::ExternalUsageSuspected;
    }
    if sample.capacity_credits > center * (1.0 + CLUSTER_BAND_RATIO) {
        match state.pending_high.take() {
            Some(pending) if consistent_pair(&pending, &sample) => {
                state.samples = vec![pending, sample];
                state.low_streak = 0;
                OAuthQuotaIntervalStatus::ValidSample
            }
            _ => {
                state.pending_high = Some(sample);
                OAuthQuotaIntervalStatus::OutlierRejected
            }
        }
    } else {
        accept(state, sample);
        OAuthQuotaIntervalStatus::ValidSample
    }
}

fn accept(state: &mut QuotaWindowState, sample: QuotaCapacitySample) {
    state.samples.push(sample);
    if state.samples.len() > MAX_ACCEPTED_SAMPLES {
        state.samples.remove(0);
    }
    state.pending_high = None;
    state.low_streak = 0;
}

fn consistent_pair(pending: &QuotaCapacitySample, sample: &QuotaCapacitySample) -> bool {
    let low = pending.capacity_credits.min(sample.capacity_credits);
    let high = pending.capacity_credits.max(sample.capacity_credits);
    high <= low * (1.0 + CLUSTER_BAND_RATIO)
}

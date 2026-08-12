use any2api_storage::api::OAuthQuotaRequestLogUsage;

use super::{
    robust::{self, CandidateDecision},
    state::{MAX_ACCEPTED_SAMPLES, MAX_COMPETING_SAMPLES, QuotaCapacitySample, QuotaWindowState},
};
use crate::oauth::quota::types::OAuthQuotaIntervalStatus;

pub(super) const NANOS_PER_CREDIT: f64 = 1_000_000_000.0;
/// Minimum accumulated percent delta before a clean segment mints a capacity
/// sample. Bootstrapping trades precision for an early learning estimate; the
/// standard threshold keeps the denominator large against official percent
/// quantization and upstream accounting skew.
pub(super) const BOOTSTRAP_MINT_DELTA_PERCENT: f64 = 1.5;
pub(super) const STANDARD_MINT_DELTA_PERCENT: f64 = 5.0;
pub(super) const SALVAGE_DELTA_PERCENT: f64 = 1.5;
/// A purely inherited prior has no current-epoch confirmation, so two
/// consistent contradicting candidates may replace it; a confirmed model keeps
/// the four-candidate requirement from ADR-0141.
const CONFIRMED_REPLACEMENT_CANDIDATES: usize = 4;
const INHERITED_REPLACEMENT_CANDIDATES: usize = 2;

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
        state.competing_samples.clear();
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

fn classify_and_store(
    state: &mut QuotaWindowState,
    sample: QuotaCapacitySample,
) -> OAuthQuotaIntervalStatus {
    match robust::classify_candidate(&state.samples, sample.capacity_credits) {
        CandidateDecision::Accept => {
            state.competing_samples.clear();
            push_sample(&mut state.samples, sample, MAX_ACCEPTED_SAMPLES);
            OAuthQuotaIntervalStatus::ValidSample
        }
        CandidateDecision::ExternalUsage => {
            if promote_competing_cluster(state, sample) {
                OAuthQuotaIntervalStatus::ValidSample
            } else {
                OAuthQuotaIntervalStatus::ExternalUsageSuspected
            }
        }
        CandidateDecision::Outlier => {
            if promote_competing_cluster(state, sample) {
                OAuthQuotaIntervalStatus::ValidSample
            } else {
                OAuthQuotaIntervalStatus::OutlierRejected
            }
        }
    }
}

fn promote_competing_cluster(state: &mut QuotaWindowState, sample: QuotaCapacitySample) -> bool {
    let Some(center) = robust::median(&state.samples) else {
        state.competing_samples.clear();
        return false;
    };
    let candidate_is_low = sample.capacity_credits < center;
    if state
        .competing_samples
        .first()
        .is_some_and(|value| (value.capacity_credits < center) != candidate_is_low)
    {
        state.competing_samples.clear();
    }
    push_sample(&mut state.competing_samples, sample, MAX_COMPETING_SAMPLES);
    let required = if state.fresh_sample_count() == 0 {
        INHERITED_REPLACEMENT_CANDIDATES
    } else {
        CONFIRMED_REPLACEMENT_CANDIDATES
    };
    if state.competing_samples.len() >= required && robust::consistent(&state.competing_samples) {
        state.samples = std::mem::take(&mut state.competing_samples);
        true
    } else {
        false
    }
}

fn push_sample(samples: &mut Vec<QuotaCapacitySample>, sample: QuotaCapacitySample, limit: usize) {
    samples.push(sample);
    if samples.len() > limit {
        samples.remove(0);
    }
}

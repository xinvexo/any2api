use any2api_domain::QuotaCostUnit;
use any2api_storage::api::OAuthQuotaRequestLogUsage;

use super::{
    robust::{self, CandidateDecision},
    state::{MAX_COMPETING_SAMPLES, MAX_SAMPLES_PER_EPOCH, QuotaCapacitySample, QuotaWindowState},
};
use crate::oauth::quota::types::OAuthQuotaIntervalStatus;

const NANOS_PER_CREDIT: f64 = 1_000_000_000.0;

pub(super) struct LearningResult {
    pub(super) status: OAuthQuotaIntervalStatus,
    pub(super) local_cost_credits: f64,
    pub(super) unpriced_request_count: u64,
}

pub(super) fn apply_usage(
    state: &mut QuotaWindowState,
    delta_used_percent: f64,
    expected_unit: QuotaCostUnit,
    usage: OAuthQuotaRequestLogUsage,
    observed_at_ms: u64,
) -> LearningResult {
    let cost = usage.total_cost_nanos as f64 / NANOS_PER_CREDIT;
    let unpriced = usage.unpriced_request_count;
    if unpriced > 0 {
        state.competing_samples.clear();
        return result(OAuthQuotaIntervalStatus::UnpricedUsage, cost, unpriced);
    }
    if usage.unit != Some(expected_unit) || usage.priced_request_count == 0 || cost <= 0.0 {
        state.competing_samples.clear();
        return result(
            OAuthQuotaIntervalStatus::ExternalUsageSuspected,
            cost,
            unpriced,
        );
    }
    let candidate = cost * 100.0 / delta_used_percent;
    if !candidate.is_finite() || candidate <= 0.0 {
        state.competing_samples.clear();
        return result(OAuthQuotaIntervalStatus::Invalid, cost, unpriced);
    }
    let sample = QuotaCapacitySample {
        capacity_credits: candidate,
        observed_at_ms,
        rate_cards: usage.rate_cards,
    };
    let status = match robust::classify_candidate(&state.samples, candidate) {
        CandidateDecision::Accept => {
            state.competing_samples.clear();
            push_sample(&mut state.samples, sample, MAX_SAMPLES_PER_EPOCH);
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
    };
    result(status, cost, unpriced)
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
    if state.competing_samples.len() == MAX_COMPETING_SAMPLES
        && robust::stable(&state.competing_samples)
    {
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

fn result(
    status: OAuthQuotaIntervalStatus,
    local_cost_credits: f64,
    unpriced_request_count: u64,
) -> LearningResult {
    LearningResult {
        status,
        local_cost_credits,
        unpriced_request_count,
    }
}

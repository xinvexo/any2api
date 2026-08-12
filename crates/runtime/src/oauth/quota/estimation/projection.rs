use any2api_provider::api::OAuthQuotaUsage;

use super::{
    aggregate,
    state::{QuotaEstimatorState, QuotaWindowKey, QuotaWindowState},
};
use crate::oauth::quota::types::{
    OAuthQuotaEstimate, OAuthQuotaEstimateConfidence, OAuthQuotaIntervalStatus,
};

pub(in crate::oauth::quota) fn project(
    usage: &OAuthQuotaUsage,
    state: Option<&QuotaEstimatorState>,
) -> Vec<OAuthQuotaEstimate> {
    let Some(state) = state else {
        return Vec::new();
    };
    usage
        .rate_limit
        .as_ref()
        .map_or(&[][..], |rate| rate.windows.as_slice())
        .iter()
        .filter_map(|window| {
            let key = QuotaWindowKey::from_window(window);
            let state = state.windows.iter().find(|state| state.key == key)?;
            Some(project_window(window.used_percent, window.reset_at, state))
        })
        .collect()
}

fn project_window(
    used_percent: f64,
    window_reset_at: Option<i64>,
    state: &QuotaWindowState,
) -> OAuthQuotaEstimate {
    let capacity = aggregate::weighted_median(&state.samples);
    let used = capacity.map(|value| value * used_percent.clamp(0.0, 100.0) / 100.0);
    let remaining = capacity
        .zip(used)
        .map(|(capacity, used)| (capacity - used).max(0.0));
    let relative_mad = aggregate::relative_mad(&state.samples);
    let fresh_sample_count = state.fresh_sample_count();
    let confidence = confidence(state, relative_mad);
    let mut rate_cards = state
        .samples
        .iter()
        .flat_map(|sample| sample.rate_cards.iter().cloned())
        .collect::<Vec<_>>();
    rate_cards.sort();
    rate_cards.dedup();
    OAuthQuotaEstimate {
        window_id: state.key.id.clone(),
        window_kind: state.key.kind,
        limit_window_seconds: state.key.limit_window_seconds,
        window_reset_at,
        epoch: state.epoch,
        epoch_started_at: seconds(state.epoch_started_at_ms),
        confidence,
        estimated_capacity_credits: capacity,
        estimated_used_credits: used,
        estimated_remaining_credits: remaining,
        sample_count: u32::try_from(state.samples.len()).unwrap_or(u32::MAX),
        fresh_sample_count: u32::try_from(fresh_sample_count).unwrap_or(u32::MAX),
        relative_mad,
        latest_interval: state.latest_interval.clone(),
        rate_cards,
    }
}

const STABLE_MINIMUM_SAMPLES: usize = 3;
const STABLE_MAX_RELATIVE_MAD: f64 = 0.20;

/// Confidence is derived, not a state machine: how many measurements exist,
/// how tightly they agree, and whether the latest interval kept telemetry
/// integrity.
fn confidence(state: &QuotaWindowState, relative_mad: Option<f64>) -> OAuthQuotaEstimateConfidence {
    if state.samples.is_empty() {
        return OAuthQuotaEstimateConfidence::Unknown;
    }
    if degrades(state.latest_interval.status) {
        return OAuthQuotaEstimateConfidence::Degraded;
    }
    if state.samples.len() >= STABLE_MINIMUM_SAMPLES
        && relative_mad.is_some_and(|value| value <= STABLE_MAX_RELATIVE_MAD)
    {
        OAuthQuotaEstimateConfidence::Stable
    } else {
        OAuthQuotaEstimateConfidence::Learning
    }
}

fn degrades(status: OAuthQuotaIntervalStatus) -> bool {
    matches!(
        status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
            | OAuthQuotaIntervalStatus::UnpricedUsage
            | OAuthQuotaIntervalStatus::Invalid
    )
}

fn seconds(milliseconds: u64) -> i64 {
    i64::try_from(milliseconds / 1_000).unwrap_or(i64::MAX)
}

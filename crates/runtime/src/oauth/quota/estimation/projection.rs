use any2api_provider::api::OAuthQuotaUsage;

use super::state::{QuotaEstimatorState, QuotaWindowKey, QuotaWindowState};
use crate::oauth::quota::types::OAuthQuotaEstimate;

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
    let capacity = state.capacity_credits();
    let used = capacity.map(|value| value * used_percent.clamp(0.0, 100.0) / 100.0);
    let remaining = capacity
        .zip(used)
        .map(|(capacity, used)| (capacity - used).max(0.0));
    OAuthQuotaEstimate {
        window_id: state.key.id.clone(),
        window_kind: state.key.kind,
        limit_window_seconds: state.key.limit_window_seconds,
        window_reset_at,
        estimated_capacity_credits: capacity,
        estimated_used_credits: used,
        estimated_remaining_credits: remaining,
        completed_interval_count: state.completed_interval_count,
    }
}

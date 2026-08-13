use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_provider::api::OAuthQuotaWindow;

use super::{OAuthQuotaEstimator, state::QuotaWindowState, transition};
use crate::request_telemetry::QuotaObservationBoundary;

const NANOS_PER_CREDIT: f64 = 1_000_000_000.0;

pub(super) async fn observe(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    window: &OAuthQuotaWindow,
    mut state: QuotaWindowState,
    expected_unit: QuotaCostUnit,
    observation: &QuotaObservationBoundary,
) -> QuotaWindowState {
    let current_anchor = transition::anchor(window, observation.position);
    if state.anchor.position.process_id != observation.position.process_id
        || state.anchor.position.sequence > observation.position.sequence
    {
        state.anchor = current_anchor;
        return state;
    }

    let delta_used_percent = window.used_percent - state.anchor.used_percent;
    if !delta_used_percent.is_finite() {
        return state;
    }
    if delta_used_percent <= 0.0 {
        return state;
    }

    let local_cost_nanos = estimator
        .repository
        .oauth_quota_local_cost_nanos(
            id,
            state.anchor.position,
            observation.position,
            expected_unit,
        )
        .await
        .unwrap_or_default();
    if local_cost_nanos == 0 {
        return state;
    }

    let local_cost_credits = local_cost_nanos as f64 / NANOS_PER_CREDIT;
    let total_delta_used_percent = state.total_delta_used_percent + delta_used_percent;
    let total_local_cost_credits = state.total_local_cost_credits + local_cost_credits;
    let completed_interval_count = state.completed_interval_count.saturating_add(1);
    let capacity_credits = total_local_cost_credits * 100.0 / total_delta_used_percent;
    if !capacity_credits.is_finite() || capacity_credits <= 0.0 {
        return state;
    }

    state.total_delta_used_percent = total_delta_used_percent;
    state.total_local_cost_credits = total_local_cost_credits;
    state.completed_interval_count = completed_interval_count;
    state.anchor = current_anchor;
    state
}

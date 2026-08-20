use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_storage::api::StorageError;

use super::{
    OAuthQuotaEstimator,
    state::{OfficialQuotaCycle, QuotaWindowKey, QuotaWindowState},
};
use crate::request_telemetry::QuotaObservationBoundary;

pub(super) struct WindowMeasurement<'a> {
    pub(super) key: QuotaWindowKey,
    pub(super) cycle: OfficialQuotaCycle,
    pub(super) previous: Option<&'a QuotaWindowState>,
    pub(super) used_percent: f64,
    pub(super) credits_takeover: bool,
    pub(super) capacity_eligible: bool,
}

pub(super) async fn measure(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    measurement: WindowMeasurement<'_>,
    expected_unit: QuotaCostUnit,
    observation: &QuotaObservationBoundary,
) -> Result<QuotaWindowState, StorageError> {
    let WindowMeasurement {
        key,
        cycle,
        previous,
        used_percent,
        credits_takeover,
        capacity_eligible,
    } = measurement;
    let previous = previous.filter(|state| state.matches_cycle(cycle));
    if let Some(previous) = previous.filter(|state| state.credits_takeover) {
        return Ok(QuotaWindowState::measured(
            key,
            cycle,
            previous.estimated_included_cost_nanos,
            used_percent,
            true,
            capacity_eligible,
        ));
    }
    let local_cost_nanos = if cycle.started_at_ms > observation.observed_at_ms {
        0
    } else {
        estimator
            .repository
            .oauth_quota_window_local_cost_nanos(
                id,
                cycle.started_at_ms,
                observation.observed_at_ms,
                observation.position,
                expected_unit,
            )
            .await?
    };
    let estimated_included_cost_nanos = if credits_takeover {
        if capacity_eligible {
            previous
                .and_then(QuotaWindowState::capacity_cost_nanos)
                .map(|capacity| capacity.min(local_cost_nanos))
        } else {
            None
        }
    } else {
        Some(local_cost_nanos)
    };
    Ok(QuotaWindowState::measured(
        key,
        cycle,
        estimated_included_cost_nanos,
        used_percent,
        credits_takeover,
        capacity_eligible,
    ))
}

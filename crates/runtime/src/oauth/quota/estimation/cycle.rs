use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_storage::api::StorageError;

use super::{
    OAuthQuotaEstimator,
    state::{OfficialQuotaCycle, QuotaWindowKey, QuotaWindowState},
};
use crate::request_telemetry::QuotaObservationBoundary;

pub(super) async fn measure(
    estimator: &OAuthQuotaEstimator,
    id: OAuthAccountId,
    key: QuotaWindowKey,
    cycle: OfficialQuotaCycle,
    capacity_eligible: bool,
    expected_unit: QuotaCostUnit,
    observation: &QuotaObservationBoundary,
) -> Result<QuotaWindowState, StorageError> {
    if cycle.started_at_ms > observation.observed_at_ms {
        return Ok(QuotaWindowState::measured(key, cycle, 0, capacity_eligible));
    }
    let local_cost_nanos = estimator
        .repository
        .oauth_quota_window_local_cost_nanos(
            id,
            cycle.started_at_ms,
            observation.observed_at_ms,
            observation.position,
            expected_unit,
        )
        .await?;
    Ok(QuotaWindowState::measured(
        key,
        cycle,
        local_cost_nanos,
        capacity_eligible,
    ))
}

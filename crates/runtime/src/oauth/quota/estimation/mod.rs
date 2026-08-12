mod interval;
mod learning;
mod projection;
mod robust;
pub(super) mod state;
mod transition;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_provider::api::OAuthQuotaUsage;
use any2api_storage::api::OAuthQuotaEstimationRepository;

use self::state::{QuotaEstimatorState, QuotaWindowKey};
use super::types::{OAuthQuotaEstimate, OAuthQuotaIntervalStatus};
use crate::request_telemetry::{RequestTelemetry, RequestTelemetryObservation};

pub(super) struct OAuthQuotaEstimator {
    repository: Arc<dyn OAuthQuotaEstimationRepository>,
}

pub(super) struct EstimationResult {
    pub(super) state: QuotaEstimatorState,
    pub(super) estimates: Vec<OAuthQuotaEstimate>,
}

impl OAuthQuotaEstimator {
    pub(super) fn new(repository: Arc<dyn OAuthQuotaEstimationRepository>) -> Self {
        Self { repository }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn observe(
        &self,
        id: OAuthAccountId,
        usage: &OAuthQuotaUsage,
        previous: Option<QuotaEstimatorState>,
        credential_fingerprint: String,
        expected_unit: QuotaCostUnit,
        observation: RequestTelemetryObservation,
        telemetry: Option<&RequestTelemetry>,
    ) -> EstimationResult {
        let mut state =
            previous.unwrap_or_else(|| QuotaEstimatorState::new(credential_fingerprint.clone()));
        let identity_changed = state.credential_fingerprint != credential_fingerprint;
        if identity_changed {
            state.credential_fingerprint = credential_fingerprint;
            state.windows.clear();
        }
        let current_windows = usage
            .rate_limit
            .as_ref()
            .map_or(&[][..], |rate| rate.windows.as_slice());
        let mut next_windows = Vec::with_capacity(current_windows.len());
        for window in current_windows {
            let key = QuotaWindowKey::from_window(window);
            let previous = state.windows.iter().find(|value| value.key == key).cloned();
            let key_changed =
                previous.is_none() && state.windows.iter().any(|value| value.key.id == key.id);
            let window_state = match previous {
                Some(previous) if !transition::must_reset(&previous, window, &observation) => {
                    interval::observe(
                        self,
                        id,
                        window,
                        previous,
                        expected_unit,
                        observation.clone(),
                        telemetry,
                    )
                    .await
                }
                Some(mut previous) => {
                    learning::salvage_segment(&mut previous);
                    transition::rollover_window(
                        previous,
                        state.allocate_epoch(),
                        window,
                        observation.clone(),
                    )
                }
                None => transition::new_window(
                    key,
                    state.allocate_epoch(),
                    window,
                    observation.clone(),
                    if identity_changed || key_changed {
                        OAuthQuotaIntervalStatus::ResetBoundary
                    } else {
                        OAuthQuotaIntervalStatus::AwaitingBaseline
                    },
                ),
            };
            next_windows.push(window_state);
        }
        state.windows = next_windows;
        let estimates = projection::project(usage, Some(&state));
        EstimationResult { state, estimates }
    }
}

pub(in crate::oauth::quota) use projection::project;

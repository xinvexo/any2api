use any2api_domain::RoutingCredentialId;
use any2api_runtime::api::{UpstreamCredentialRequestOutcome, UpstreamCredentialUsageSummary};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(super) struct UpstreamCredentialUsageResponse {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    recent_outcomes: Vec<UpstreamCredentialRequestOutcomeResponse>,
}

impl UpstreamCredentialUsageResponse {
    pub(super) fn for_id(
        id: RoutingCredentialId,
        usage: &[UpstreamCredentialUsageSummary],
    ) -> Self {
        let Some(summary) = usage.iter().find(|summary| summary.id == id) else {
            return Self {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                recent_outcomes: Vec::new(),
            };
        };
        Self {
            total_requests: summary.total_requests,
            successful_requests: summary.successful_requests,
            failed_requests: summary.failed_requests(),
            recent_outcomes: summary
                .recent_outcomes
                .iter()
                .map(UpstreamCredentialRequestOutcomeResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct UpstreamCredentialRequestOutcomeResponse {
    status_code: u16,
}

impl From<&UpstreamCredentialRequestOutcome> for UpstreamCredentialRequestOutcomeResponse {
    fn from(value: &UpstreamCredentialRequestOutcome) -> Self {
        Self {
            status_code: value.status_code,
        }
    }
}

pub(super) async fn load(state: &AppState) -> Vec<UpstreamCredentialUsageSummary> {
    match state.request_telemetry().upstream_credential_usage().await {
        Ok(usage) => usage,
        Err(error) => {
            tracing::warn!(%error, "upstream credential usage statistics unavailable");
            Vec::new()
        }
    }
}

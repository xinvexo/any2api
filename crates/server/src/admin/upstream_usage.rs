use std::time::{SystemTime, UNIX_EPOCH};

use any2api_domain::RoutingCredentialId;
use any2api_runtime::api::{
    UpstreamCredentialUsageSummary, UpstreamCredentialWindowSlot, empty_upstream_window_slots,
};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(super) struct UpstreamCredentialUsageResponse {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    window_minutes: u64,
    window_slots: Vec<UpstreamCredentialWindowSlotResponse>,
}

impl UpstreamCredentialUsageResponse {
    pub(super) fn for_id(
        id: RoutingCredentialId,
        usage: &[UpstreamCredentialUsageSummary],
    ) -> Self {
        let Some(summary) = usage.iter().find(|summary| summary.id == id) else {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(0))
                .unwrap_or(0);
            return Self {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                window_minutes: any2api_runtime::api::UPSTREAM_USAGE_WINDOW_MINUTES,
                window_slots: empty_upstream_window_slots(now_ms)
                    .into_iter()
                    .map(UpstreamCredentialWindowSlotResponse::from)
                    .collect(),
            };
        };
        Self {
            total_requests: summary.total_requests,
            successful_requests: summary.successful_requests,
            failed_requests: summary.failed_requests(),
            window_minutes: any2api_runtime::api::UPSTREAM_USAGE_WINDOW_MINUTES,
            window_slots: summary
                .window_slots
                .iter()
                .map(UpstreamCredentialWindowSlotResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct UpstreamCredentialWindowSlotResponse {
    started_at_ms: u64,
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
}

impl From<&UpstreamCredentialWindowSlot> for UpstreamCredentialWindowSlotResponse {
    fn from(value: &UpstreamCredentialWindowSlot) -> Self {
        Self {
            started_at_ms: value.started_at_ms,
            total_requests: value.total_requests,
            successful_requests: value.successful_requests,
            failed_requests: value
                .total_requests
                .saturating_sub(value.successful_requests),
        }
    }
}

impl From<UpstreamCredentialWindowSlot> for UpstreamCredentialWindowSlotResponse {
    fn from(value: UpstreamCredentialWindowSlot) -> Self {
        Self::from(&value)
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

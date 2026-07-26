use any2api_domain::RoutingCredentialId;
use any2api_runtime::api::UpstreamCredentialUsageSummary;

use crate::{admin::request_usage::RequestUsageResponse, state::AppState};

pub(super) fn for_id(
    id: RoutingCredentialId,
    usage: &[UpstreamCredentialUsageSummary],
) -> RequestUsageResponse {
    let Some(summary) = usage.iter().find(|summary| summary.id == id) else {
        return RequestUsageResponse::empty();
    };
    RequestUsageResponse::new(
        summary.total_requests,
        summary.successful_requests,
        &summary.window_slots,
    )
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

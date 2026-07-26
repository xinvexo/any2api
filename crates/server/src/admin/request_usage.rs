use std::time::{SystemTime, UNIX_EPOCH};

use any2api_runtime::api::{
    REQUEST_USAGE_WINDOW_MINUTES, RequestUsageWindowSlot, empty_request_usage_window_slots,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct RequestUsageResponse {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    window_minutes: u64,
    window_slots: Vec<RequestUsageWindowSlotResponse>,
}

impl RequestUsageResponse {
    pub(crate) fn new(
        total_requests: u64,
        successful_requests: u64,
        window_slots: &[RequestUsageWindowSlot],
    ) -> Self {
        Self {
            total_requests,
            successful_requests,
            failed_requests: total_requests.saturating_sub(successful_requests),
            window_minutes: REQUEST_USAGE_WINDOW_MINUTES,
            window_slots: window_slots
                .iter()
                .map(RequestUsageWindowSlotResponse::from)
                .collect(),
        }
    }

    pub(crate) fn empty() -> Self {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(0))
            .unwrap_or(0);
        Self::new(0, 0, &empty_request_usage_window_slots(now_ms))
    }
}

#[derive(Debug, Serialize)]
struct RequestUsageWindowSlotResponse {
    started_at_ms: u64,
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
}

impl From<&RequestUsageWindowSlot> for RequestUsageWindowSlotResponse {
    fn from(value: &RequestUsageWindowSlot) -> Self {
        Self {
            started_at_ms: value.started_at_ms,
            total_requests: value.total_requests,
            successful_requests: value.successful_requests,
            failed_requests: value.failed_requests(),
        }
    }
}

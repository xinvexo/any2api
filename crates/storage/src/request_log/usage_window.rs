use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::StorageError;

/// Each request usage bar covers this many minutes.
pub const REQUEST_USAGE_WINDOW_MINUTES: u64 = 2;
/// Fixed bars for the last hour (30 x 2 min), newest-last; empty = gray.
pub const REQUEST_USAGE_WINDOW_COUNT: usize = 30;

pub(crate) const REQUEST_USAGE_WINDOW_MS: u64 = REQUEST_USAGE_WINDOW_MINUTES * 60 * 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestUsageWindowSlot {
    pub started_at_ms: u64,
    pub total_requests: u64,
    pub successful_requests: u64,
}

impl RequestUsageWindowSlot {
    #[must_use]
    pub fn failed_requests(&self) -> u64 {
        self.total_requests.saturating_sub(self.successful_requests)
    }
}

/// Build a fixed-length newest-last window for admin responses with no usage row.
#[must_use]
pub fn empty_request_usage_window_slots(now_ms: u64) -> Vec<RequestUsageWindowSlot> {
    build_request_usage_window_slots(now_ms, &HashMap::new())
}

pub(crate) fn build_request_usage_window_slots(
    now_ms: u64,
    filled: &HashMap<u64, (u64, u64)>,
) -> Vec<RequestUsageWindowSlot> {
    let newest = align_request_usage_window_start(now_ms);
    let oldest =
        newest.saturating_sub((REQUEST_USAGE_WINDOW_COUNT as u64 - 1) * REQUEST_USAGE_WINDOW_MS);
    (0..REQUEST_USAGE_WINDOW_COUNT)
        .map(|index| {
            let started_at_ms = oldest + (index as u64) * REQUEST_USAGE_WINDOW_MS;
            let (total_requests, successful_requests) =
                filled.get(&started_at_ms).copied().unwrap_or((0, 0));
            RequestUsageWindowSlot {
                started_at_ms,
                total_requests,
                successful_requests,
            }
        })
        .collect()
}

pub(crate) fn request_usage_window_range_start(now_ms: u64) -> u64 {
    align_request_usage_window_start(now_ms)
        .saturating_sub((REQUEST_USAGE_WINDOW_COUNT as u64 - 1) * REQUEST_USAGE_WINDOW_MS)
}

pub(crate) fn unix_now_ms() -> Result<u64, StorageError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .map_err(|_| StorageError::CorruptTelemetry)
}

fn align_request_usage_window_start(now_ms: u64) -> u64 {
    (now_ms / REQUEST_USAGE_WINDOW_MS) * REQUEST_USAGE_WINDOW_MS
}

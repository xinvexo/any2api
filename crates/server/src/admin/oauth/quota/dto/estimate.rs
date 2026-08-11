use any2api_runtime::api::{
    OAuthQuotaEstimate, OAuthQuotaEstimateConfidence, OAuthQuotaIntervalDiagnostic,
    OAuthQuotaIntervalStatus, OAuthQuotaWindowKind,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct OAuthQuotaEstimateResponse {
    window_id: String,
    window_kind: &'static str,
    limit_window_seconds: Option<u64>,
    window_reset_at: Option<i64>,
    epoch: u64,
    epoch_started_at: i64,
    confidence: &'static str,
    estimated_capacity_credits: Option<f64>,
    estimated_used_credits: Option<f64>,
    estimated_remaining_credits: Option<f64>,
    sample_count: u32,
    relative_mad: Option<f64>,
    latest_interval: OAuthQuotaIntervalDiagnosticResponse,
    rate_cards: Vec<String>,
}

impl From<OAuthQuotaEstimate> for OAuthQuotaEstimateResponse {
    fn from(value: OAuthQuotaEstimate) -> Self {
        Self {
            window_id: value.window_id,
            window_kind: window_kind(value.window_kind),
            limit_window_seconds: value.limit_window_seconds,
            window_reset_at: value.window_reset_at,
            epoch: value.epoch,
            epoch_started_at: value.epoch_started_at,
            confidence: confidence(value.confidence),
            estimated_capacity_credits: value.estimated_capacity_credits,
            estimated_used_credits: value.estimated_used_credits,
            estimated_remaining_credits: value.estimated_remaining_credits,
            sample_count: value.sample_count,
            relative_mad: value.relative_mad,
            latest_interval: value.latest_interval.into(),
            rate_cards: value.rate_cards,
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaIntervalDiagnosticResponse {
    status: &'static str,
    started_at: Option<i64>,
    ended_at: i64,
    delta_used_percent: Option<f64>,
    local_cost_credits: Option<f64>,
    unpriced_request_count: u64,
    queue_dropped_request_logs: u64,
    storage_failed_request_logs: u64,
    pruned_request_logs: u64,
}

impl From<OAuthQuotaIntervalDiagnostic> for OAuthQuotaIntervalDiagnosticResponse {
    fn from(value: OAuthQuotaIntervalDiagnostic) -> Self {
        Self {
            status: interval_status(value.status),
            started_at: value.started_at,
            ended_at: value.ended_at,
            delta_used_percent: value.delta_used_percent,
            local_cost_credits: value.local_cost_credits,
            unpriced_request_count: value.unpriced_request_count,
            queue_dropped_request_logs: value.queue_dropped_request_logs,
            storage_failed_request_logs: value.storage_failed_request_logs,
            pruned_request_logs: value.pruned_request_logs,
        }
    }
}

fn confidence(value: OAuthQuotaEstimateConfidence) -> &'static str {
    match value {
        OAuthQuotaEstimateConfidence::Unknown => "unknown",
        OAuthQuotaEstimateConfidence::Learning => "learning",
        OAuthQuotaEstimateConfidence::Stable => "stable",
        OAuthQuotaEstimateConfidence::Degraded => "degraded",
    }
}

fn interval_status(value: OAuthQuotaIntervalStatus) -> &'static str {
    match value {
        OAuthQuotaIntervalStatus::AwaitingBaseline => "awaiting_baseline",
        OAuthQuotaIntervalStatus::NoChange => "no_change",
        OAuthQuotaIntervalStatus::ValidSample => "valid_sample",
        OAuthQuotaIntervalStatus::ResetBoundary => "reset_boundary",
        OAuthQuotaIntervalStatus::TelemetryIncomplete => "telemetry_incomplete",
        OAuthQuotaIntervalStatus::UnpricedUsage => "unpriced_usage",
        OAuthQuotaIntervalStatus::ExternalUsageSuspected => "external_usage_suspected",
        OAuthQuotaIntervalStatus::OutlierRejected => "outlier_rejected",
        OAuthQuotaIntervalStatus::Invalid => "invalid",
    }
}

fn window_kind(value: OAuthQuotaWindowKind) -> &'static str {
    match value {
        OAuthQuotaWindowKind::Time => "time",
        OAuthQuotaWindowKind::Credits => "credits",
    }
}

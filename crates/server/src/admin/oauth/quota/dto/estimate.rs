use any2api_runtime::api::{OAuthQuotaEstimate, OAuthQuotaWindowKind};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct OAuthQuotaEstimateResponse {
    window_id: String,
    window_kind: &'static str,
    limit_window_seconds: Option<u64>,
    window_reset_at: Option<i64>,
    estimated_capacity_credits: Option<f64>,
    estimated_used_credits: Option<f64>,
    estimated_remaining_credits: Option<f64>,
}

impl From<OAuthQuotaEstimate> for OAuthQuotaEstimateResponse {
    fn from(value: OAuthQuotaEstimate) -> Self {
        Self {
            window_id: value.window_id,
            window_kind: window_kind(value.window_kind),
            limit_window_seconds: value.limit_window_seconds,
            window_reset_at: value.window_reset_at,
            estimated_capacity_credits: value.estimated_capacity_credits,
            estimated_used_credits: value.estimated_used_credits,
            estimated_remaining_credits: value.estimated_remaining_credits,
        }
    }
}

fn window_kind(value: OAuthQuotaWindowKind) -> &'static str {
    match value {
        OAuthQuotaWindowKind::Time => "time",
        OAuthQuotaWindowKind::Credits => "credits",
    }
}

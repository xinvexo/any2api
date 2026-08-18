use any2api_runtime::api::{
    RequestLogOverview, RequestLogOverviewBucket, RequestLogOverviewModel,
    RequestLogOverviewTotals, SystemMetricsSnapshot,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "OverviewUsageResponse.ts"))]
pub(crate) struct OverviewUsageResponse {
    generated_at_ms: u64,
    range: String,
    range_started_at_ms: u64,
    range_ended_at_ms: u64,
    retained_started_at_ms: Option<u64>,
    retained: OverviewUsageTotalsResponse,
    selected: OverviewUsageTotalsResponse,
    time_buckets: Vec<OverviewUsageTimeBucketResponse>,
    models: Vec<OverviewUsageModelResponse>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "OverviewResourcesResponse.ts")
)]
pub(crate) struct OverviewResourcesResponse {
    sampled_at_ms: u64,
    process: OverviewProcessResourcesResponse,
    system: OverviewSystemResourcesResponse,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "OverviewProcessResourcesResponse.ts")
)]
struct OverviewProcessResourcesResponse {
    resident_memory_bytes: u64,
    cpu_usage_percent: f32,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "OverviewSystemResourcesResponse.ts")
)]
struct OverviewSystemResourcesResponse {
    used_memory_bytes: u64,
    total_memory_bytes: u64,
    cpu_usage_percent: f32,
}

impl From<SystemMetricsSnapshot> for OverviewResourcesResponse {
    fn from(value: SystemMetricsSnapshot) -> Self {
        Self {
            sampled_at_ms: value.sampled_at_ms,
            process: OverviewProcessResourcesResponse {
                resident_memory_bytes: value.process_resident_memory_bytes,
                cpu_usage_percent: value.process_cpu_usage_percent,
            },
            system: OverviewSystemResourcesResponse {
                used_memory_bytes: value.system_used_memory_bytes,
                total_memory_bytes: value.system_total_memory_bytes,
                cpu_usage_percent: value.system_cpu_usage_percent,
            },
        }
    }
}

impl From<RequestLogOverview> for OverviewUsageResponse {
    fn from(value: RequestLogOverview) -> Self {
        Self {
            generated_at_ms: value.generated_at_ms,
            range: value.range.as_str().to_owned(),
            range_started_at_ms: value.range_started_at_ms,
            range_ended_at_ms: value.range_ended_at_ms,
            retained_started_at_ms: value.retained_started_at_ms,
            retained: value.retained_totals.into(),
            selected: value.range_totals.into(),
            time_buckets: value.time_buckets.into_iter().map(Into::into).collect(),
            models: value.models.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "OverviewUsageTotalsResponse.ts")
)]
struct OverviewUsageTotalsResponse {
    request_count: u64,
    successful_request_count: u64,
    failed_request_count: u64,
    token_usage_request_count: u64,
    input_tokens: String,
    output_tokens: String,
    cache_read_tokens: String,
    total_tokens: String,
}

impl From<RequestLogOverviewTotals> for OverviewUsageTotalsResponse {
    fn from(value: RequestLogOverviewTotals) -> Self {
        Self {
            request_count: value.request_count,
            successful_request_count: value.successful_request_count,
            failed_request_count: value.failed_request_count(),
            token_usage_request_count: value.token_usage_request_count,
            input_tokens: value.input_tokens.to_string(),
            output_tokens: value.output_tokens.to_string(),
            cache_read_tokens: value.cache_read_tokens.to_string(),
            total_tokens: value.total_tokens().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "OverviewUsageTimeBucketResponse.ts")
)]
struct OverviewUsageTimeBucketResponse {
    started_at_ms: u64,
    ended_at_ms: u64,
    request_count: u64,
    successful_request_count: u64,
    failed_request_count: u64,
}

impl From<RequestLogOverviewBucket> for OverviewUsageTimeBucketResponse {
    fn from(value: RequestLogOverviewBucket) -> Self {
        Self {
            started_at_ms: value.started_at_ms,
            ended_at_ms: value.ended_at_ms,
            request_count: value.request_count,
            successful_request_count: value.successful_request_count,
            failed_request_count: value.failed_request_count(),
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "OverviewUsageModelResponse.ts")
)]
struct OverviewUsageModelResponse {
    public_model: Option<String>,
    is_other: bool,
    request_count: u64,
    successful_request_count: u64,
    failed_request_count: u64,
    token_usage_request_count: u64,
    input_tokens: String,
    output_tokens: String,
    cache_read_tokens: String,
    total_tokens: String,
}

impl From<RequestLogOverviewModel> for OverviewUsageModelResponse {
    fn from(value: RequestLogOverviewModel) -> Self {
        let totals: OverviewUsageTotalsResponse = value.totals.into();
        Self {
            public_model: value.public_model,
            is_other: value.is_other,
            request_count: totals.request_count,
            successful_request_count: totals.successful_request_count,
            failed_request_count: totals.failed_request_count,
            token_usage_request_count: totals.token_usage_request_count,
            input_tokens: totals.input_tokens,
            output_tokens: totals.output_tokens,
            cache_read_tokens: totals.cache_read_tokens,
            total_tokens: totals.total_tokens,
        }
    }
}

#[cfg(test)]
pub(in crate::admin) fn export_bindings(config: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    use ts_rs::TS as _;

    OverviewUsageResponse::export_all(config)?;
    OverviewResourcesResponse::export_all(config)
}

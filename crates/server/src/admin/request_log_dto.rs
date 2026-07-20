use any2api_domain::{CompletedRequestLog, RequestAttempt, RequestLog};
use any2api_runtime::api::RequestTelemetryMetrics;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct RequestLogListResponse {
    items: Vec<RequestLogResponse>,
    telemetry: RequestTelemetryResponse,
}

impl RequestLogListResponse {
    pub(crate) fn new(logs: Vec<RequestLog>, metrics: RequestTelemetryMetrics) -> Self {
        Self {
            items: logs.into_iter().map(RequestLogResponse::from).collect(),
            telemetry: metrics.into(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct RequestLogDetailResponse {
    request: RequestLogResponse,
    attempts: Vec<RequestAttemptResponse>,
    telemetry: RequestTelemetryResponse,
}

impl RequestLogDetailResponse {
    pub(crate) fn new(record: CompletedRequestLog, metrics: RequestTelemetryMetrics) -> Self {
        Self {
            request: record.request.into(),
            attempts: record
                .attempts
                .into_iter()
                .map(RequestAttemptResponse::from)
                .collect(),
            telemetry: metrics.into(),
        }
    }
}

#[derive(Serialize)]
struct RequestTelemetryResponse {
    queued_records: usize,
    dropped_records: u64,
    persisted_records: u64,
}

impl From<RequestTelemetryMetrics> for RequestTelemetryResponse {
    fn from(value: RequestTelemetryMetrics) -> Self {
        Self {
            queued_records: value.queued_records,
            dropped_records: value.dropped_records,
            persisted_records: value.persisted_records,
        }
    }
}

#[derive(Serialize)]
struct RequestLogResponse {
    request_id: String,
    started_at_ms: u64,
    config_revision: u64,
    gateway_api_key_id: Option<String>,
    ingress_protocol: &'static str,
    operation: &'static str,
    public_model: Option<String>,
    provider_endpoint_id: Option<String>,
    credential_id: Option<String>,
    proxy_profile_id: Option<String>,
    status_code: u16,
    error_class: Option<&'static str>,
    attempt_count: u32,
    latency_ms: u64,
    first_token_ms: Option<u64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    is_stream: bool,
}

impl From<RequestLog> for RequestLogResponse {
    fn from(value: RequestLog) -> Self {
        Self {
            request_id: value.request_id.to_string(),
            started_at_ms: value.started_at_ms,
            config_revision: value.config_revision.get(),
            gateway_api_key_id: value.gateway_api_key_id.map(|id| id.to_string()),
            ingress_protocol: value.ingress_protocol.as_str(),
            operation: value.operation.as_str(),
            public_model: value.public_model,
            provider_endpoint_id: value.provider_endpoint_id.map(|id| id.to_string()),
            credential_id: value.credential_id.map(|id| id.to_string()),
            proxy_profile_id: value.proxy_profile_id.map(|id| id.to_string()),
            status_code: value.status_code,
            error_class: value.error_class.map(|class| class.as_str()),
            attempt_count: value.attempt_count,
            latency_ms: value.latency_ms,
            first_token_ms: value.first_token_ms,
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            cache_read_tokens: value.cache_read_tokens,
            cache_write_tokens: value.cache_write_tokens,
            is_stream: value.is_stream,
        }
    }
}

#[derive(Serialize)]
struct RequestAttemptResponse {
    attempt_no: u32,
    route_target_id: Option<String>,
    credential_id: Option<String>,
    proxy_profile_id: Option<String>,
    started_at_ms: u64,
    duration_ms: u64,
    retry_safety: Option<&'static str>,
    error_class: Option<&'static str>,
    status_code: Option<u16>,
    outcome: &'static str,
}

impl From<RequestAttempt> for RequestAttemptResponse {
    fn from(value: RequestAttempt) -> Self {
        Self {
            attempt_no: value.attempt_no,
            route_target_id: value.route_target_id.map(|id| id.to_string()),
            credential_id: value.credential_id.map(|id| id.to_string()),
            proxy_profile_id: value.proxy_profile_id.map(|id| id.to_string()),
            started_at_ms: value.started_at_ms,
            duration_ms: value.duration_ms,
            retry_safety: value.retry_safety.map(|safety| safety.as_str()),
            error_class: value.error_class.map(|class| class.as_str()),
            status_code: value.status_code,
            outcome: value.outcome.as_str(),
        }
    }
}

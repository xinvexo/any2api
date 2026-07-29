use any2api_domain::{CompletedRequestLog, RequestAttempt, RequestLog};
use any2api_runtime::api::{PublishedSnapshot, RequestTelemetryMetrics};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct RequestLogListResponse {
    items: Vec<RequestLogResponse>,
    telemetry: RequestTelemetryResponse,
}

impl RequestLogListResponse {
    pub(crate) fn new(
        logs: Vec<RequestLog>,
        metrics: RequestTelemetryMetrics,
        snapshot: &PublishedSnapshot,
    ) -> Self {
        Self {
            items: logs
                .into_iter()
                .map(|log| RequestLogResponse::from_log(log, snapshot))
                .collect(),
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
    pub(crate) fn new(
        record: CompletedRequestLog,
        metrics: RequestTelemetryMetrics,
        snapshot: &PublishedSnapshot,
    ) -> Self {
        Self {
            request: RequestLogResponse::from_log(record.request, snapshot),
            attempts: record
                .attempts
                .into_iter()
                .map(|attempt| RequestAttemptResponse::from_attempt(attempt, snapshot))
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
    client_ip: String,
    config_revision: u64,
    gateway_api_key_id: Option<String>,
    ingress_protocol: &'static str,
    operation: &'static str,
    public_model: Option<String>,
    thinking_level: Option<String>,
    provider_endpoint_id: Option<String>,
    provider_endpoint_name: Option<String>,
    credential_id: Option<String>,
    credential_label: Option<String>,
    oauth_account_id: Option<String>,
    oauth_account_label: Option<String>,
    proxy_profile_id: Option<String>,
    proxy_profile_label: Option<String>,
    status_code: u16,
    error_message: Option<String>,
    attempt_count: u32,
    latency_ms: u64,
    first_token_ms: Option<u64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    is_stream: bool,
}

impl RequestLogResponse {
    fn from_log(value: RequestLog, snapshot: &PublishedSnapshot) -> Self {
        let provider_endpoint_name = value.provider_endpoint_id.and_then(|id| {
            snapshot
                .provider_endpoints()
                .get(id)
                .map(|endpoint| endpoint.name().to_owned())
        });
        let credential_label = value.credential_id.and_then(|id| {
            snapshot
                .provider_credentials()
                .get(id)
                .map(|credential| credential.label().to_owned())
        });
        let oauth_account_label = value.oauth_account_id.and_then(|id| {
            snapshot
                .oauth_accounts()
                .get(id)
                .map(|account| account.label().to_owned())
        });
        let proxy_profile_label = value.proxy_profile_id.and_then(|id| {
            snapshot
                .proxies()
                .get(id)
                .map(|profile| profile.name().to_owned())
        });
        Self::from_parts(
            value,
            provider_endpoint_name,
            credential_label,
            oauth_account_label,
            proxy_profile_label,
        )
    }

    fn from_parts(
        value: RequestLog,
        provider_endpoint_name: Option<String>,
        credential_label: Option<String>,
        oauth_account_label: Option<String>,
        proxy_profile_label: Option<String>,
    ) -> Self {
        Self {
            request_id: value.request_id.to_string(),
            started_at_ms: value.started_at_ms,
            client_ip: value.client_ip.to_string(),
            config_revision: value.config_revision.get(),
            gateway_api_key_id: value.gateway_api_key_id.map(|id| id.to_string()),
            ingress_protocol: value.ingress_protocol.as_str(),
            operation: value.operation.as_str(),
            public_model: value.public_model,
            thinking_level: value.thinking_level,
            provider_endpoint_id: value.provider_endpoint_id.map(|id| id.to_string()),
            provider_endpoint_name,
            credential_id: value.credential_id.map(|id| id.to_string()),
            credential_label,
            oauth_account_id: value.oauth_account_id.map(|id| id.to_string()),
            oauth_account_label,
            proxy_profile_id: value.proxy_profile_id.map(|id| id.to_string()),
            proxy_profile_label,
            status_code: value.status_code,
            error_message: value.error_message,
            attempt_count: value.attempt_count,
            latency_ms: value.latency_ms,
            first_token_ms: value.first_token_ms,
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            cache_read_tokens: value.cache_read_tokens,
            is_stream: value.is_stream,
        }
    }
}

#[derive(Serialize)]
struct RequestAttemptResponse {
    attempt_no: u32,
    route_target_id: Option<String>,
    credential_id: Option<String>,
    credential_label: Option<String>,
    oauth_account_id: Option<String>,
    oauth_account_label: Option<String>,
    proxy_profile_id: Option<String>,
    proxy_profile_label: Option<String>,
    started_at_ms: u64,
    duration_ms: u64,
    error_message: Option<String>,
    status_code: Option<u16>,
}

impl RequestAttemptResponse {
    fn from_attempt(value: RequestAttempt, snapshot: &PublishedSnapshot) -> Self {
        let credential_label = value.credential_id.and_then(|id| {
            snapshot
                .provider_credentials()
                .get(id)
                .map(|credential| credential.label().to_owned())
        });
        let oauth_account_label = value.oauth_account_id.and_then(|id| {
            snapshot
                .oauth_accounts()
                .get(id)
                .map(|account| account.label().to_owned())
        });
        let proxy_profile_label = value.proxy_profile_id.and_then(|id| {
            snapshot
                .proxies()
                .get(id)
                .map(|profile| profile.name().to_owned())
        });
        Self {
            attempt_no: value.attempt_no,
            route_target_id: value.route_target_id.map(|id| id.to_string()),
            credential_id: value.credential_id.map(|id| id.to_string()),
            credential_label,
            oauth_account_id: value.oauth_account_id.map(|id| id.to_string()),
            oauth_account_label,
            proxy_profile_id: value.proxy_profile_id.map(|id| id.to_string()),
            proxy_profile_label,
            started_at_ms: value.started_at_ms,
            duration_ms: value.duration_ms,
            error_message: value.error_message,
            status_code: value.status_code,
        }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{
        ConfigRevision, ErrorClass, ProtocolDialect, ProtocolOperation, RequestId, RequestLog,
    };

    use super::RequestLogResponse;

    #[test]
    fn response_preserves_exact_token_telemetry_and_labels() {
        let response = RequestLogResponse::from_parts(
            RequestLog {
                request_id: RequestId::new(),
                started_at_ms: 1,
                client_ip: "203.0.113.8".parse().expect("client IP"),
                config_revision: ConfigRevision::INITIAL,
                gateway_api_key_id: None,
                ingress_protocol: ProtocolDialect::OpenAiResponses,
                operation: ProtocolOperation::Responses,
                public_model: Some("codex-local".into()),
                thinking_level: Some("high".into()),
                provider_endpoint_id: None,
                credential_id: None,
                oauth_account_id: None,
                proxy_profile_id: None,
                status_code: 200,
                error_class: Some(ErrorClass::Upstream),
                error_message: Some("The upstream model was not found".into()),
                attempt_count: 1,
                latency_ms: 30,
                first_token_ms: Some(18),
                input_tokens: Some(120),
                output_tokens: Some(45),
                cache_read_tokens: Some(30),
                is_stream: true,
            },
            Some("Codex upstream".into()),
            Some("Primary Codex".into()),
            Some("work-oauth".into()),
            Some("DIRECT".into()),
        );

        let json = serde_json::to_value(response).expect("request log response JSON");
        assert_eq!(json["first_token_ms"], 18);
        assert_eq!(json["input_tokens"], 120);
        assert_eq!(json["output_tokens"], 45);
        assert_eq!(json["cache_read_tokens"], 30);
        assert!(json.get("cache_write_tokens").is_none());
        assert_eq!(json["thinking_level"], "high");
        assert_eq!(json["client_ip"], "203.0.113.8");
        assert_eq!(json["provider_endpoint_name"], "Codex upstream");
        assert_eq!(json["credential_label"], "Primary Codex");
        assert_eq!(json["oauth_account_label"], "work-oauth");
        assert_eq!(json["proxy_profile_label"], "DIRECT");
        assert_eq!(json["error_message"], "The upstream model was not found");
        assert!(json.get("error_class").is_none());
    }
}

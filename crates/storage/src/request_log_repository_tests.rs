use any2api_domain::{
    CompletedRequestLog, ConfigRevision, CredentialId, GatewayApiKeyDraft, GatewayApiKeyId,
    MAX_TOKEN_COUNT, ProtocolDialect, ProtocolOperation, ProviderEndpointId, ProxyProfileId,
    RequestAttempt, RequestAttemptOutcome, RequestId, RequestLog, RetrySafety, RouteTargetId,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use tempfile::tempdir;

use crate::{
    gateway_api_key_repository::GatewayApiKeyRepository,
    gateway_api_key_usage_repository::{
        GATEWAY_API_KEY_RECENT_OUTCOME_LIMIT, GatewayApiKeyUsageRepository,
    },
    request_log_repository::RequestLogRepository,
    sqlite::SqliteStore,
    vault::SecretBytes,
};

#[tokio::test]
async fn request_log_and_attempt_round_trip_without_requiring_live_config_references() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("request-logs.sqlite3"))
        .await
        .expect("storage");
    let request_id = RequestId::new();
    let mut record = record(request_id, 1_000, true);
    record.request.status_code = 401;
    record.request.error_class = Some(any2api_domain::ErrorClass::Authentication);
    record.request.error_message = Some("upstream authentication failed".into());
    record.attempts[0].error_class = Some(any2api_domain::ErrorClass::Authentication);
    record.attempts[0].error_message = Some("Incorrect API key provided".into());
    record.attempts[0].status_code = Some(401);
    record.attempts[0].outcome = RequestAttemptOutcome::UpstreamError;

    store
        .append_request_logs(std::slice::from_ref(&record))
        .await
        .expect("append request log");

    let listed = store.list_request_logs(10).await.expect("list logs");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].request_id, request_id);
    assert_eq!(listed[0].attempt_count, 1);
    assert_eq!(listed[0].gateway_api_key_id, None);
    assert_eq!(listed[0].provider_endpoint_id, None);
    assert_eq!(listed[0].credential_id, None);
    assert_eq!(listed[0].proxy_profile_id, None);
    assert_eq!(
        listed[0].error_message.as_deref(),
        Some("upstream authentication failed")
    );

    let loaded = store
        .get_request_log(request_id)
        .await
        .expect("get log")
        .expect("stored log");
    assert_eq!(loaded.request.request_id, request_id);
    assert_eq!(loaded.attempts.len(), 1);
    assert_eq!(loaded.attempts[0].attempt_no, 1);
    assert_eq!(loaded.attempts[0].route_target_id, None);
    assert_eq!(
        loaded.attempts[0].outcome,
        RequestAttemptOutcome::UpstreamError
    );
    assert_eq!(
        loaded.request.error_message.as_deref(),
        Some("upstream authentication failed")
    );
    assert_eq!(
        loaded.attempts[0].error_message.as_deref(),
        Some("Incorrect API key provided")
    );
    assert_eq!(loaded.request.first_token_ms, Some(12));
    assert_eq!(loaded.request.input_tokens, Some(120));
    assert_eq!(loaded.request.output_tokens, Some(45));
    assert_eq!(loaded.request.cache_read_tokens, Some(30));
    assert_eq!(loaded.request.cache_write_tokens, Some(6));
}

#[tokio::test]
async fn request_log_round_trip_preserves_null_zero_and_max_safe_telemetry() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("telemetry-boundaries.sqlite3"))
        .await
        .expect("storage");
    let request_id = RequestId::new();
    let mut record = record(request_id, 1_000, false);
    record.request.first_token_ms = None;
    record.request.input_tokens = Some(0);
    record.request.output_tokens = Some(MAX_TOKEN_COUNT);
    record.request.cache_read_tokens = None;
    record.request.cache_write_tokens = Some(0);

    store
        .append_request_logs(std::slice::from_ref(&record))
        .await
        .expect("append request log");

    let loaded = store
        .get_request_log(request_id)
        .await
        .expect("get log")
        .expect("stored log")
        .request;
    assert_eq!(loaded.first_token_ms, None);
    assert_eq!(loaded.input_tokens, Some(0));
    assert_eq!(loaded.output_tokens, Some(MAX_TOKEN_COUNT));
    assert_eq!(loaded.cache_read_tokens, None);
    assert_eq!(loaded.cache_write_tokens, Some(0));
}

#[tokio::test]
async fn retention_and_row_limits_delete_parent_and_child_rows_in_batches() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("retention.sqlite3"))
        .await
        .expect("storage");
    let first = record(RequestId::new(), 100, false);
    let second = record(RequestId::new(), 200, false);
    let third = record(RequestId::new(), 300, false);
    store
        .append_request_logs(&[first.clone(), second.clone(), third.clone()])
        .await
        .expect("append logs");

    assert_eq!(
        store
            .prune_request_logs(250, 10, 100)
            .await
            .expect("retention prune"),
        2
    );
    assert_eq!(store.list_request_logs(10).await.expect("list").len(), 1);
    assert!(
        store
            .get_request_log(first.request.request_id)
            .await
            .expect("get")
            .is_none()
    );

    let fourth = record(RequestId::new(), 400, false);
    store
        .append_request_logs(std::slice::from_ref(&fourth))
        .await
        .expect("append fourth");
    assert_eq!(
        store
            .prune_request_logs(0, 1, 100)
            .await
            .expect("row prune"),
        1
    );
    let remaining = store.list_request_logs(10).await.expect("remaining");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].request_id, fourth.request.request_id);
}

#[tokio::test]
async fn gateway_key_usage_aggregates_final_requests_and_limits_recent_outcomes() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("gateway-usage.sqlite3"))
        .await
        .expect("storage");
    let first_id = GatewayApiKeyId::new();
    let first_configuration = store
        .create_gateway_api_key(
            ConfigRevision::INITIAL,
            first_id,
            GatewayApiKeyDraft::new("Desktop", true).expect("first draft"),
            gateway_token(b'a'),
        )
        .await
        .expect("create first key");
    let second_id = GatewayApiKeyId::new();
    store
        .create_gateway_api_key(
            first_configuration.revision(),
            second_id,
            GatewayApiKeyDraft::new("Laptop", true).expect("second draft"),
            gateway_token(b'b'),
        )
        .await
        .expect("create second key");

    let mut records = (0..=GATEWAY_API_KEY_RECENT_OUTCOME_LIMIT)
        .map(|index| {
            let status = if index == 0 {
                500
            } else if index == GATEWAY_API_KEY_RECENT_OUTCOME_LIMIT {
                429
            } else {
                200
            };
            usage_record(first_id, 100 + u64::from(index), status)
        })
        .collect::<Vec<_>>();
    records.push(usage_record(second_id, 1_000, 503));
    let mut anonymous = record(RequestId::new(), 2_000, false);
    anonymous.request.gateway_api_key_id = None;
    records.push(anonymous);
    store
        .append_request_logs(&records)
        .await
        .expect("append usage logs");

    let usage = store
        .list_gateway_api_key_usage()
        .await
        .expect("gateway usage");
    let first = usage
        .iter()
        .find(|summary| summary.id == first_id)
        .expect("first usage");
    assert_eq!(first.total_requests, 25);
    assert_eq!(first.successful_requests, 23);
    assert_eq!(first.failed_requests(), 2);
    assert_eq!(first.recent_outcomes.len(), 24);
    assert_eq!(
        first.recent_outcomes.first().map(|item| item.status_code),
        Some(200)
    );
    assert_eq!(
        first.recent_outcomes.last().map(|item| item.status_code),
        Some(429)
    );

    let second = usage
        .iter()
        .find(|summary| summary.id == second_id)
        .expect("second usage");
    assert_eq!(second.total_requests, 1);
    assert_eq!(second.successful_requests, 0);
    assert_eq!(second.failed_requests(), 1);
    assert_eq!(second.recent_outcomes[0].status_code, 503);
}

fn record(request_id: RequestId, started_at_ms: u64, with_attempt: bool) -> CompletedRequestLog {
    let attempts = with_attempt
        .then(|| RequestAttempt {
            request_id,
            attempt_no: 1,
            route_target_id: Some(RouteTargetId::new()),
            credential_id: Some(CredentialId::new()),
            oauth_account_id: None,
            proxy_profile_id: Some(ProxyProfileId::new()),
            started_at_ms: started_at_ms + 1,
            duration_ms: 25,
            retry_safety: Some(RetrySafety::Ambiguous),
            error_class: None,
            error_message: None,
            status_code: Some(200),
            outcome: RequestAttemptOutcome::Success,
        })
        .into_iter()
        .collect();
    CompletedRequestLog {
        request: RequestLog {
            request_id,
            started_at_ms,
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: Some(GatewayApiKeyId::new()),
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("codex-test".into()),
            provider_endpoint_id: Some(ProviderEndpointId::new()),
            credential_id: Some(CredentialId::new()),
            oauth_account_id: None,
            proxy_profile_id: Some(ProxyProfileId::new()),
            status_code: 200,
            error_class: None,
            error_message: None,
            attempt_count: u32::from(with_attempt),
            latency_ms: 30,
            first_token_ms: Some(12),
            input_tokens: Some(120),
            output_tokens: Some(45),
            cache_read_tokens: Some(30),
            cache_write_tokens: Some(6),
            is_stream: true,
        },
        attempts,
    }
}

fn usage_record(
    gateway_api_key_id: GatewayApiKeyId,
    started_at_ms: u64,
    status_code: u16,
) -> CompletedRequestLog {
    let mut record = record(RequestId::new(), started_at_ms, false);
    record.request.gateway_api_key_id = Some(gateway_api_key_id);
    record.request.status_code = status_code;
    record
}

fn gateway_token(byte: u8) -> SecretBytes {
    format!("a2k_v1_{}", URL_SAFE_NO_PAD.encode([byte; 32]))
        .into_bytes()
        .into()
}

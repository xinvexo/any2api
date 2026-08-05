use any2api_domain::{
    CompletedRequestLog, ConfigRevision, CredentialId, ErrorClass, GatewayApiKeyDraft,
    GatewayApiKeyId, MAX_REQUEST_LOG_ROWS, MAX_TOKEN_COUNT, ProtocolDialect, ProtocolOperation,
    ProviderEndpointId, ProxyProfileId, RequestAttempt, RequestAttemptOutcome, RequestId,
    RequestLog, RetrySafety, RouteTargetId,
};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigurationMutation, commit_configuration},
    gateway_api_key::GatewayApiKeyUsageRepository,
    request_log::{REQUEST_USAGE_WINDOW_COUNT, REQUEST_USAGE_WINDOW_MINUTES, RequestLogRepository},
    secret::SecretBytes,
    sqlite::SqliteStore,
};

mod capacity;
mod corruption;
mod pagination;

const USAGE_WINDOW_MS: u64 = REQUEST_USAGE_WINDOW_MINUTES * 60 * 1_000;

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
    record.attempts[0].error_message = Some("upstream returned HTTP 401 (authentication)".into());
    record.attempts[0].status_code = Some(401);
    record.attempts[0].outcome = RequestAttemptOutcome::UpstreamError;

    store
        .append_request_logs(std::slice::from_ref(&record), MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append request log");

    let listed = store
        .list_request_logs(0, None, 10)
        .await
        .expect("list logs");
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0].request_id, request_id);
    assert_eq!(
        listed.items[0].client_ip,
        "2001:db8::1"
            .parse::<std::net::IpAddr>()
            .expect("client IP")
    );
    assert_eq!(listed.items[0].attempt_count, 1);
    assert_eq!(listed.items[0].gateway_api_key_id, None);
    assert_eq!(listed.items[0].provider_endpoint_id, None);
    assert_eq!(listed.items[0].credential_id, None);
    assert_eq!(listed.items[0].proxy_profile_id, None);
    assert_eq!(
        listed.items[0].error_message.as_deref(),
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
        Some("upstream returned HTTP 401 (authentication)")
    );
    assert_eq!(loaded.request.first_token_ms, Some(12));
    assert_eq!(loaded.request.input_tokens, Some(120));
    assert_eq!(loaded.request.output_tokens, Some(45));
    assert_eq!(loaded.request.cache_read_tokens, Some(30));
}

#[tokio::test]
async fn request_log_round_trip_preserves_optional_zero_and_max_safe_telemetry() {
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

    store
        .append_request_logs(std::slice::from_ref(&record), MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append request log");

    let loaded = store
        .get_request_log(request_id)
        .await
        .expect("get log")
        .expect("stored log")
        .request;
    assert_eq!(loaded.first_token_ms, None);
    assert_eq!(
        loaded.client_ip,
        "2001:db8::1"
            .parse::<std::net::IpAddr>()
            .expect("client IP")
    );
    assert_eq!(loaded.input_tokens, Some(0));
    assert_eq!(loaded.output_tokens, Some(MAX_TOKEN_COUNT));
    assert_eq!(loaded.cache_read_tokens, None);
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
        .append_request_logs(
            &[first.clone(), second.clone(), third.clone()],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append logs");

    assert_eq!(
        store
            .prune_request_logs(250, 10, 100)
            .await
            .expect("retention prune")
            .deleted_rows(),
        2
    );
    assert_eq!(
        store
            .list_request_logs(0, None, 10)
            .await
            .expect("list")
            .items
            .len(),
        1
    );
    assert!(
        store
            .get_request_log(first.request.request_id)
            .await
            .expect("get")
            .is_none()
    );

    let fourth = record(RequestId::new(), 400, false);
    store
        .append_request_logs(std::slice::from_ref(&fourth), MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append fourth");
    assert_eq!(
        store
            .prune_request_logs(0, 1, 100)
            .await
            .expect("row prune")
            .deleted_rows(),
        1
    );
    let remaining = store
        .list_request_logs(0, None, 10)
        .await
        .expect("remaining");
    assert_eq!(remaining.total, 1);
    assert_eq!(remaining.items[0].request_id, fourth.request.request_id);
}

#[tokio::test]
async fn gateway_key_usage_aggregates_final_requests_and_fills_time_windows() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("gateway-usage.sqlite3"))
        .await
        .expect("storage");
    let first_id = GatewayApiKeyId::new();
    let first_configuration = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateGatewayApiKey {
            id: first_id,
            draft: GatewayApiKeyDraft::new("Desktop", true).expect("first draft"),
            token: gateway_token(b'a'),
        },
    )
    .await
    .expect("create first key");
    let second_id = GatewayApiKeyId::new();
    commit_configuration(
        &store,
        first_configuration.revision(),
        ConfigurationMutation::CreateGatewayApiKey {
            id: second_id,
            draft: GatewayApiKeyDraft::new("Laptop", true).expect("second draft"),
            token: gateway_token(b'b'),
        },
    )
    .await
    .expect("create second key");

    let now_bucket = align_usage_window_now();
    let mut records = vec![
        usage_record(first_id, now_bucket + 1_000, 200),
        usage_record(first_id, now_bucket + 2_000, 500),
        usage_record(
            first_id,
            now_bucket.saturating_sub(USAGE_WINDOW_MS) + 500,
            204,
        ),
        // Outside the visible hour, but still part of the retention-window totals.
        usage_record(
            first_id,
            now_bucket.saturating_sub(USAGE_WINDOW_MS * (REQUEST_USAGE_WINDOW_COUNT as u64 + 2)),
            429,
        ),
        usage_record(second_id, now_bucket + 3_000, 503),
    ];
    let mut failed_stream = usage_record(first_id, now_bucket + 2_500, 200);
    failed_stream.request.error_class = Some(ErrorClass::Upstream);
    records.push(failed_stream);
    let mut anonymous = record(RequestId::new(), now_bucket + 4_000, false);
    anonymous.request.gateway_api_key_id = None;
    records.push(anonymous);
    store
        .append_request_logs(&records, MAX_REQUEST_LOG_ROWS)
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
    assert_eq!(first.total_requests, 5);
    assert_eq!(first.successful_requests, 2);
    assert_eq!(first.failed_requests(), 3);
    assert_eq!(first.window_slots.len(), REQUEST_USAGE_WINDOW_COUNT);
    assert_eq!(
        first.window_slots.last().map(|slot| slot.started_at_ms),
        Some(now_bucket)
    );
    let newest = first.window_slots.last().expect("newest slot");
    assert_eq!(newest.total_requests, 3);
    assert_eq!(newest.successful_requests, 1);
    assert_eq!(newest.failed_requests(), 2);
    let previous = &first.window_slots[REQUEST_USAGE_WINDOW_COUNT - 2];
    assert_eq!(previous.started_at_ms, now_bucket - USAGE_WINDOW_MS);
    assert_eq!(previous.total_requests, 1);
    assert_eq!(previous.successful_requests, 1);
    assert!(
        first
            .window_slots
            .iter()
            .take(REQUEST_USAGE_WINDOW_COUNT - 2)
            .all(|slot| slot.total_requests == 0)
    );

    let second = usage
        .iter()
        .find(|summary| summary.id == second_id)
        .expect("second usage");
    assert_eq!(second.total_requests, 1);
    assert_eq!(second.successful_requests, 0);
    assert_eq!(second.failed_requests(), 1);
    assert_eq!(second.window_slots.len(), REQUEST_USAGE_WINDOW_COUNT);
    let second_newest = second.window_slots.last().expect("second newest slot");
    assert_eq!(second_newest.total_requests, 1);
    assert_eq!(second_newest.successful_requests, 0);
    assert_eq!(second_newest.failed_requests(), 1);
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
            client_ip: "2001:db8::1".parse().expect("client IP"),
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: Some(GatewayApiKeyId::new()),
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("codex-test".into()),
            thinking_level: None,
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

fn align_usage_window_now() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    (now / USAGE_WINDOW_MS) * USAGE_WINDOW_MS
}

fn gateway_token(seed: u8) -> SecretBytes {
    let alphabet = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let ch = char::from(alphabet[usize::from(seed) % alphabet.len()]);
    format!(
        "{}{}",
        any2api_domain::GATEWAY_TOKEN_PREFIX,
        ch.to_string()
            .repeat(any2api_domain::GATEWAY_TOKEN_BODY_LEN)
    )
    .into_bytes()
    .into()
}

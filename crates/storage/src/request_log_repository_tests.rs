use any2api_domain::{
    CompletedRequestLog, ConfigRevision, CredentialId, GatewayApiKeyId, ProtocolDialect,
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, RequestAttempt, RequestAttemptOutcome,
    RequestId, RequestLog, RetrySafety, RouteTargetId,
};
use tempfile::tempdir;

use crate::{request_log_repository::RequestLogRepository, sqlite::SqliteStore};

#[tokio::test]
async fn request_log_and_attempt_round_trip_without_requiring_live_config_references() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("request-logs.sqlite3"))
        .await
        .expect("storage");
    let request_id = RequestId::new();
    let record = record(request_id, 1_000, true);

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

    let loaded = store
        .get_request_log(request_id)
        .await
        .expect("get log")
        .expect("stored log");
    assert_eq!(loaded.request.request_id, request_id);
    assert_eq!(loaded.attempts.len(), 1);
    assert_eq!(loaded.attempts[0].attempt_no, 1);
    assert_eq!(loaded.attempts[0].route_target_id, None);
    assert_eq!(loaded.attempts[0].outcome, RequestAttemptOutcome::Success);
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

fn record(request_id: RequestId, started_at_ms: u64, with_attempt: bool) -> CompletedRequestLog {
    let attempts = with_attempt
        .then(|| RequestAttempt {
            request_id,
            attempt_no: 1,
            route_target_id: Some(RouteTargetId::new()),
            credential_id: Some(CredentialId::new()),
            proxy_profile_id: Some(ProxyProfileId::new()),
            started_at_ms: started_at_ms + 1,
            duration_ms: 25,
            retry_safety: Some(RetrySafety::Ambiguous),
            error_class: None,
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
            proxy_profile_id: Some(ProxyProfileId::new()),
            status_code: 200,
            error_class: None,
            attempt_count: u32::from(with_attempt),
            latency_ms: 30,
            first_token_ms: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            is_stream: false,
        },
        attempts,
    }
}

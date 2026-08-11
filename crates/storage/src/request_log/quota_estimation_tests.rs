use any2api_domain::{
    CompletedRequestLog, ConfigRevision, MAX_REQUEST_LOG_ROWS, OAuthAccountDraft, OAuthAccountId,
    ProtocolDialect, ProtocolOperation, ProviderKind, ProxyProfileId, RequestId, RequestLog,
};
use tempfile::tempdir;

use crate::{
    api::{
        OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION, OAuthAccountDocument, OAuthQuotaEstimationRepository,
        OAuthQuotaSnapshotRepository, RequestLogRepository, SqliteStore, StoredOAuthQuotaSnapshot,
    },
    configuration::{ConfigurationMutation, commit_configuration},
};

#[tokio::test]
async fn quota_usage_filters_account_and_window_and_groups_complete_models() {
    let (_directory, store, id) = store_with_account().await;
    let other_id = OAuthAccountId::new();
    let mut records = vec![
        record(id, 999, "gpt-5.5", Some(900), Some(90), Some(9)),
        record(id, 1_000, "gpt-5.5", Some(100), Some(10), Some(20)),
        record(id, 1_500, "gpt-5.5", Some(50), Some(5), None),
        record(id, 1_999, "gpt-5.4", Some(200), Some(20), Some(40)),
        record(id, 2_000, "gpt-5.5", Some(800), Some(80), Some(8)),
    ];
    records.push(record(
        other_id,
        1_250,
        "gpt-5.5",
        Some(700),
        Some(70),
        None,
    ));
    store
        .append_request_logs(&records, MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append request logs");

    let usage = store
        .oauth_quota_request_log_usage(id, 1_000, 2_000)
        .await
        .expect("quota log usage");

    assert!(usage.records_complete);
    assert_eq!(usage.models.len(), 2);
    assert_eq!(usage.models[0].public_model, "gpt-5.4");
    assert_eq!(usage.models[0].input_tokens, 200);
    assert_eq!(usage.models[0].output_tokens, 20);
    assert_eq!(usage.models[0].cache_read_tokens, 40);
    assert_eq!(usage.models[1].public_model, "gpt-5.5");
    assert_eq!(usage.models[1].input_tokens, 150);
    assert_eq!(usage.models[1].output_tokens, 15);
    assert_eq!(usage.models[1].cache_read_tokens, 20);
}

#[tokio::test]
async fn incomplete_usage_is_reported_and_reset_boundary_deletes_snapshot_atomically() {
    let (_directory, store, id) = store_with_account().await;
    let incomplete = record(id, 1_500, "gpt-5.5", Some(50), None, None);
    store
        .append_request_logs(&[incomplete], MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append incomplete log");
    let usage = store
        .oauth_quota_request_log_usage(id, 1_000, 2_000)
        .await
        .expect("quota log usage");
    assert!(!usage.records_complete);

    store
        .upsert_oauth_quota_snapshot(&StoredOAuthQuotaSnapshot {
            oauth_account_id: id,
            schema_version: OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
            fetched_at: 1,
            payload: br#"{}"#.to_vec(),
        })
        .await
        .expect("quota snapshot");
    store
        .record_oauth_quota_reset(id, 1_600)
        .await
        .expect("record reset");
    store
        .record_oauth_quota_reset(id, 1_550)
        .await
        .expect("older reset cannot move boundary backward");

    assert_eq!(
        store
            .load_oauth_quota_reset_boundary(id)
            .await
            .expect("reset boundary"),
        Some(1_600)
    );
    assert!(
        store
            .load_oauth_quota_snapshot(id)
            .await
            .expect("quota snapshot")
            .is_none()
    );
}

async fn store_with_account() -> (tempfile::TempDir, SqliteStore, OAuthAccountId) {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("quota-estimation.sqlite3"))
        .await
        .expect("storage");
    let id = OAuthAccountId::new();
    commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateOAuthAccount {
            id,
            provider_kind: ProviderKind::Codex,
            draft: OAuthAccountDraft::new("OAuth", None, true).expect("OAuth draft"),
            safe_account_email: None,
            expires_at: None,
            models: vec!["gpt-5.5".into()],
            document: OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"access_token":"access","refresh_token":"refresh","id_token":null,"account_id":null,"email":null}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth document"),
        },
    )
    .await
    .expect("create OAuth account");
    (directory, store, id)
}

fn record(
    id: OAuthAccountId,
    started_at_ms: u64,
    model: &str,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
) -> CompletedRequestLog {
    CompletedRequestLog {
        request: RequestLog {
            request_id: RequestId::new(),
            started_at_ms,
            client_ip: "127.0.0.1".parse().expect("client IP"),
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: None,
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some(model.into()),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: Some(id),
            proxy_profile_id: Some(ProxyProfileId::DIRECT),
            status_code: 200,
            error_class: None,
            error_message: None,
            attempt_count: 1,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            is_stream: false,
        },
        attempts: Vec::new(),
    }
}

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, CredentialId, CredentialKind, MAX_REQUEST_LOG_ROWS,
    OAuthAccountDraft, OAuthAccountId, ProtocolDialect, ProtocolOperation, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyProfileId, RequestAttempt,
    RequestAttemptOutcome, RequestId, RequestLog, RoutingCredentialId,
};
use tempfile::tempdir;

use crate::{
    api::{
        OAuthAccountDocument, RequestLogRepository, SecretBytes, SqliteStore,
        UpstreamCredentialUsageRepository,
    },
    configuration::{ConfigurationMutation, commit_configuration},
    request_log::{REQUEST_USAGE_WINDOW_COUNT, REQUEST_USAGE_WINDOW_MINUTES},
};

const WINDOW_MS: u64 = REQUEST_USAGE_WINDOW_MINUTES * 60 * 1_000;

#[tokio::test]
async fn usage_keeps_provider_and_oauth_sources_distinct_and_fills_window_slots() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("upstream-usage.sqlite3"))
        .await
        .expect("storage");
    let credential_id = CredentialId::new();
    let oauth_account_id = OAuthAccountId::from_uuid(*credential_id.as_uuid());
    let endpoint_id = ProviderEndpointId::new();
    let endpoint = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: ProviderEndpointDraft::new(
                "Codex",
                ProviderKind::Codex,
                "https://api.example.com/v1",
                ProtocolDialect::OpenAiResponses,
                true,
            )
            .expect("endpoint draft"),
        },
    )
    .await
    .expect("create endpoint");
    let credential = commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: ProviderCredentialDraft::new(
                "API Key",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                None,
                true,
            )
            .expect("credential draft"),
            api_key: SecretBytes::from(b"sk-test-credential".to_vec()),
        },
    )
    .await
    .expect("create credential");
    commit_configuration(
        &store,
        credential.revision(),
        ConfigurationMutation::CreateOAuthAccount {
            id: oauth_account_id,
            provider_kind: ProviderKind::Codex,
            draft: OAuthAccountDraft::new(
                "OAuth",
                None,
                true,
            )
            .expect("OAuth draft"),
            safe_account_email: None,
            expires_at: None,
            models: vec!["gpt-test".into()],
            document: OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"access_token":"access-secret","refresh_token":"refresh-secret","id_token":null,"account_id":null,"email":null}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth document"),
        },
    )
        .await
        .expect("create OAuth account");

    let now_bucket = align_now();
    let mut records = vec![
        usage_record(
            RoutingCredentialId::provider_credential(credential_id),
            endpoint_id,
            now_bucket + 1_000,
            200,
        ),
        usage_record(
            RoutingCredentialId::provider_credential(credential_id),
            endpoint_id,
            now_bucket + 2_000,
            500,
        ),
        usage_record(
            RoutingCredentialId::provider_credential(credential_id),
            endpoint_id,
            now_bucket.saturating_sub(WINDOW_MS) + 500,
            200,
        ),
        // Outside the visible window — still counted in totals, not in slots.
        usage_record(
            RoutingCredentialId::provider_credential(credential_id),
            endpoint_id,
            now_bucket.saturating_sub(WINDOW_MS * (REQUEST_USAGE_WINDOW_COUNT as u64 + 2)),
            429,
        ),
    ];
    records.push(usage_record(
        RoutingCredentialId::oauth_account(oauth_account_id),
        endpoint_id,
        now_bucket + 100,
        503,
    ));
    let mut retried = usage_record(
        RoutingCredentialId::oauth_account(oauth_account_id),
        endpoint_id,
        now_bucket + 200,
        200,
    );
    retried.attempts.push(RequestAttempt {
        request_id: retried.request.request_id,
        attempt_no: 1,
        route_target_id: None,
        credential_id: Some(credential_id),
        oauth_account_id: None,
        proxy_profile_id: Some(ProxyProfileId::DIRECT),
        started_at_ms: now_bucket + 150,
        duration_ms: 1,
        retry_safety: None,
        error_class: None,
        error_message: None,
        status_code: Some(500),
        outcome: RequestAttemptOutcome::UpstreamError,
    });
    retried.request.attempt_count = 1;
    records.push(retried);
    records.push(usage_record_without_upstream(now_bucket + 300));
    store
        .append_request_logs(&records, MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append request logs");

    let usage = store
        .list_upstream_credential_usage()
        .await
        .expect("upstream usage");
    assert_eq!(usage.len(), 2);
    let provider = usage
        .iter()
        .find(|summary| summary.id == RoutingCredentialId::provider_credential(credential_id))
        .expect("provider usage");
    assert_eq!(provider.total_requests, 4);
    assert_eq!(provider.successful_requests, 2);
    assert_eq!(provider.failed_requests(), 2);
    assert_eq!(provider.window_slots.len(), REQUEST_USAGE_WINDOW_COUNT);
    assert_eq!(
        provider.window_slots.last().map(|slot| slot.started_at_ms),
        Some(now_bucket)
    );
    let newest = provider.window_slots.last().expect("newest slot");
    assert_eq!(newest.total_requests, 2);
    assert_eq!(newest.successful_requests, 1);
    let previous = &provider.window_slots[REQUEST_USAGE_WINDOW_COUNT - 2];
    assert_eq!(previous.started_at_ms, now_bucket.saturating_sub(WINDOW_MS));
    assert_eq!(previous.total_requests, 1);
    assert_eq!(previous.successful_requests, 1);
    assert!(
        provider
            .window_slots
            .iter()
            .take(REQUEST_USAGE_WINDOW_COUNT - 2)
            .all(|slot| slot.total_requests == 0)
    );

    let oauth = usage
        .iter()
        .find(|summary| summary.id == RoutingCredentialId::oauth_account(oauth_account_id))
        .expect("OAuth usage");
    assert_eq!(oauth.total_requests, 2);
    assert_eq!(oauth.successful_requests, 1);
    assert_eq!(oauth.failed_requests(), 1);
    assert_eq!(oauth.window_slots.len(), REQUEST_USAGE_WINDOW_COUNT);
    let oauth_newest = oauth.window_slots.last().expect("oauth newest");
    assert_eq!(oauth_newest.total_requests, 2);
    assert_eq!(oauth_newest.successful_requests, 1);
}

fn align_now() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    (now / WINDOW_MS) * WINDOW_MS
}

fn usage_record(
    id: RoutingCredentialId,
    endpoint_id: ProviderEndpointId,
    started_at_ms: u64,
    status_code: u16,
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
            public_model: Some("gpt-test".into()),
            thinking_level: None,
            provider_endpoint_id: id.provider_credential_id().map(|_| endpoint_id),
            credential_id: id.provider_credential_id(),
            oauth_account_id: id.oauth_account_id(),
            proxy_profile_id: Some(ProxyProfileId::DIRECT),
            status_code,
            error_class: None,
            error_message: None,
            attempt_count: 0,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            is_stream: false,
        },
        attempts: Vec::new(),
    }
}

fn usage_record_without_upstream(started_at_ms: u64) -> CompletedRequestLog {
    let mut record = usage_record(
        RoutingCredentialId::provider_credential(CredentialId::new()),
        ProviderEndpointId::new(),
        started_at_ms,
        400,
    );
    record.request.provider_endpoint_id = None;
    record.request.credential_id = None;
    record
}

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, CredentialId, CredentialKind, MaxConcurrency,
    OAuthAccountDraft, OAuthAccountId, ProtocolDialect, ProtocolOperation, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyProfileId, RequestAttempt,
    RequestAttemptOutcome, RequestId, RequestLog, RoutingCredentialId,
};
use tempfile::tempdir;

use crate::{
    api::{
        ConfigurationRepository, OAuthAccountDocument, OAuthAccountRepository,
        RequestLogRepository, SecretBytes, SqliteStore, UpstreamCredentialUsageRepository,
    },
    upstream_credential_usage_repository::UPSTREAM_CREDENTIAL_RECENT_OUTCOME_LIMIT,
};

#[tokio::test]
async fn usage_keeps_provider_and_oauth_sources_distinct_and_counts_final_requests() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("upstream-usage.sqlite3"))
        .await
        .expect("storage");
    let credential_id = CredentialId::new();
    let oauth_account_id = OAuthAccountId::from_uuid(*credential_id.as_uuid());
    let endpoint_id = ProviderEndpointId::new();
    let endpoint = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex",
                ProviderKind::Codex,
                "https://api.example.com/v1",
                ProtocolDialect::OpenAiResponses,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("create endpoint");
    let credential = store
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            ProviderCredentialDraft::new(
                "API Key",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                MaxConcurrency::new(1).expect("max concurrency"),
                true,
            )
            .expect("credential draft"),
            SecretBytes::from(b"sk-test-credential".to_vec()),
        )
        .await
        .expect("create credential");
    store
        .create_oauth_account(
            credential.revision(),
            oauth_account_id,
            ProviderKind::Codex,
            OAuthAccountDraft::new(
                "OAuth",
                MaxConcurrency::new(1).expect("max concurrency"),
                true,
            )
            .expect("OAuth draft"),
            None,
            None,
            vec!["gpt-test".into()],
            OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"type":"codex","access_token":"access-secret","refresh_token":"refresh-secret"}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth document"),
        )
        .await
        .expect("create OAuth account");

    let mut records = (0..=UPSTREAM_CREDENTIAL_RECENT_OUTCOME_LIMIT)
        .map(|index| {
            let status = if index == 0 {
                500
            } else if index == UPSTREAM_CREDENTIAL_RECENT_OUTCOME_LIMIT {
                429
            } else {
                200
            };
            usage_record(
                RoutingCredentialId::provider_credential(credential_id),
                endpoint_id,
                100 + u64::from(index),
                status,
            )
        })
        .collect::<Vec<_>>();
    records.push(usage_record(
        RoutingCredentialId::oauth_account(oauth_account_id),
        endpoint_id,
        1_000,
        503,
    ));
    let mut retried = usage_record(
        RoutingCredentialId::oauth_account(oauth_account_id),
        endpoint_id,
        2_000,
        200,
    );
    retried.attempts.push(RequestAttempt {
        request_id: retried.request.request_id,
        attempt_no: 1,
        route_target_id: None,
        credential_id: Some(credential_id),
        oauth_account_id: None,
        proxy_profile_id: Some(ProxyProfileId::DIRECT),
        started_at_ms: 1_999,
        duration_ms: 1,
        retry_safety: None,
        error_class: None,
        status_code: Some(500),
        outcome: RequestAttemptOutcome::UpstreamError,
    });
    retried.request.attempt_count = 1;
    records.push(retried);
    records.push(usage_record_without_upstream(3_000));
    store
        .append_request_logs(&records)
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
    assert_eq!(provider.total_requests, 25);
    assert_eq!(provider.successful_requests, 23);
    assert_eq!(provider.failed_requests(), 2);
    assert_eq!(provider.recent_outcomes.len(), 24);
    assert_eq!(
        provider
            .recent_outcomes
            .first()
            .map(|item| item.status_code),
        Some(200)
    );
    assert_eq!(
        provider.recent_outcomes.last().map(|item| item.status_code),
        Some(429)
    );

    let oauth = usage
        .iter()
        .find(|summary| summary.id == RoutingCredentialId::oauth_account(oauth_account_id))
        .expect("OAuth usage");
    assert_eq!(oauth.total_requests, 2);
    assert_eq!(oauth.successful_requests, 1);
    assert_eq!(oauth.failed_requests(), 1);
    assert_eq!(
        oauth
            .recent_outcomes
            .iter()
            .map(|item| item.status_code)
            .collect::<Vec<_>>(),
        vec![503, 200]
    );
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
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: None,
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("gpt-test".into()),
            provider_endpoint_id: id.provider_credential_id().map(|_| endpoint_id),
            credential_id: id.provider_credential_id(),
            oauth_account_id: id.oauth_account_id(),
            proxy_profile_id: Some(ProxyProfileId::DIRECT),
            status_code,
            error_class: None,
            attempt_count: 0,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
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

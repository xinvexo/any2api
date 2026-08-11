use any2api_domain::{
    CompletedRequestLog, ConfigRevision, MAX_REQUEST_LOG_ROWS, OAuthAccountDraft, OAuthAccountId,
    ProtocolDialect, ProtocolOperation, ProviderKind, ProxyProfileId, QuotaCostUnit,
    QuotaServiceTier, RequestId, RequestLog, RequestQuotaCost,
};
use tempfile::tempdir;

use crate::{
    api::{
        OAuthAccountDocument, OAuthQuotaEstimationRepository, RequestLogRepository, SqliteStore,
    },
    configuration::{ConfigurationMutation, commit_configuration},
};

const RATE_CARD: &str = "openai_codex_credits_2026_08_11";

#[tokio::test]
async fn quota_usage_filters_account_and_interval_and_sums_frozen_costs() {
    let (_directory, store, id) = store_with_account().await;
    let other_id = OAuthAccountId::new();
    let records = vec![
        record(id, 900, 99, Some(900)),
        record(id, 999, 1, Some(100)),
        record(id, 1_500, 1, Some(50)),
        record(id, 1_900, 99, Some(200)),
        record(id, 1_990, 10, Some(800)),
        record(other_id, 1_250, 1, Some(700)),
    ];
    store
        .append_request_logs(&records, MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append request logs");

    let usage = store
        .oauth_quota_request_log_usage(id, 1_000, 2_000)
        .await
        .expect("quota log usage");

    assert_eq!(usage.unit, Some(QuotaCostUnit::CodexCredits));
    assert_eq!(usage.total_cost_nanos, 350);
    assert_eq!(usage.priced_request_count, 3);
    assert_eq!(usage.unpriced_request_count, 0);
    assert_eq!(usage.rate_cards, vec![RATE_CARD]);
}

#[tokio::test]
async fn status_is_irrelevant_but_missing_frozen_cost_is_unpriced() {
    let (_directory, store, id) = store_with_account().await;
    let mut priced_failure = record(id, 1_250, 1, Some(100));
    priced_failure.request.status_code = 502;
    let mut priced_cancelled = record(id, 1_400, 1, Some(50));
    priced_cancelled.request.error_class = Some(any2api_domain::ErrorClass::Cancelled);
    let unpriced = record(id, 1_500, 1, None);
    store
        .append_request_logs(
            &[priced_failure, priced_cancelled, unpriced],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append mixed logs");

    let usage = store
        .oauth_quota_request_log_usage(id, 1_000, 2_000)
        .await
        .expect("quota log usage");
    assert_eq!(usage.total_cost_nanos, 150);
    assert_eq!(usage.priced_request_count, 2);
    assert_eq!(usage.unpriced_request_count, 1);
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
    latency_ms: u64,
    cost_nanos: Option<u64>,
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
            public_model: Some("gpt-5.5".into()),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: Some(id),
            proxy_profile_id: Some(ProxyProfileId::DIRECT),
            status_code: 200,
            error_class: None,
            error_message: None,
            attempt_count: 1,
            latency_ms,
            first_token_ms: None,
            input_tokens: Some(1),
            output_tokens: Some(1),
            cache_read_tokens: None,
            quota_cost: cost_nanos.map(|amount| {
                RequestQuotaCost::new(
                    QuotaCostUnit::CodexCredits,
                    amount,
                    RATE_CARD,
                    QuotaServiceTier::Standard,
                )
                .expect("quota cost")
            }),
            is_stream: false,
        },
        attempts: Vec::new(),
    }
}

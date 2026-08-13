use any2api_domain::{
    CompletedRequestLog, ConfigRevision, MAX_REQUEST_LOG_ROWS, OAuthAccountDraft, OAuthAccountId,
    ProtocolDialect, ProtocolOperation, ProviderKind, ProxyProfileId, QuotaCostUnit,
    QuotaServiceTier, RequestId, RequestLog, RequestQuotaCost, RequestTelemetryPosition,
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
        record(id, 900, 99, Some(900), position(1)),
        record(id, 999, 1, Some(100), position(2)),
        record(id, 1_500, 1, Some(50), position(3)),
        record(id, 1_900, 99, Some(200), position(4)),
        record(id, 1_990, 10, Some(800), position(5)),
        record(other_id, 1_250, 1, Some(700), position(6)),
    ];
    store
        .append_request_logs(&records, MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append request logs");

    let usage = store
        .oauth_quota_local_cost_nanos(id, position(1), position(4), QuotaCostUnit::CodexCredits)
        .await
        .expect("quota log usage");

    assert_eq!(usage, 350);
}

#[tokio::test]
async fn status_is_irrelevant_and_only_local_cost_is_summed() {
    let (_directory, store, id) = store_with_account().await;
    let mut priced_failure = record(id, 1_250, 1, Some(100), position(1));
    priced_failure.request.status_code = 502;
    let mut priced_cancelled = record(id, 1_400, 1, Some(50), position(2));
    priced_cancelled.request.error_class = Some(any2api_domain::ErrorClass::Cancelled);
    let unpriced = record(id, 1_500, 1, None, position(3));
    store
        .append_request_logs(
            &[priced_failure, priced_cancelled, unpriced],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append mixed logs");

    let usage = store
        .oauth_quota_local_cost_nanos(id, position(0), position(3), QuotaCostUnit::CodexCredits)
        .await
        .expect("quota log usage");
    assert_eq!(usage, 150);
}

#[tokio::test]
async fn request_after_official_observation_is_excluded_regardless_of_wall_clock() {
    let (_directory, store, id) = store_with_account().await;
    let before_observation = record(id, 10_000, 500, Some(100), position(1));
    let after_observation = record(id, 1, 1, Some(900), position(2));
    store
        .append_request_logs(
            &[before_observation, after_observation],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append boundary logs");

    let usage = store
        .oauth_quota_local_cost_nanos(id, position(0), position(1), QuotaCostUnit::CodexCredits)
        .await
        .expect("usage at official observation fence");

    assert_eq!(usage, 100);
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
    telemetry_position: RequestTelemetryPosition,
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
            cache_creation_tokens: None,
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
        telemetry_position: Some(telemetry_position),
    }
}

fn position(sequence: u64) -> RequestTelemetryPosition {
    RequestTelemetryPosition {
        process_id: "44444444-4444-4444-8444-444444444444"
            .parse()
            .expect("process UUID"),
        sequence,
    }
}

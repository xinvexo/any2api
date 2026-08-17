use any2api_domain::{
    CompletedRequestLog, ConfigRevision, ErrorClass, MAX_REQUEST_LOG_ROWS, ProtocolDialect,
    ProtocolOperation, RequestId, RequestLog,
};
use tempfile::tempdir;

use crate::{request_log::RequestLogRepository, sqlite::SqliteStore};

use super::{model::RequestLogOverviewRange, query::load_request_log_overview_at};

#[tokio::test]
async fn overview_keeps_retained_and_selected_totals_separate() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("overview.sqlite3"))
        .await
        .expect("storage");
    let now_ms = 10_000_000;
    let records = [
        record(
            now_ms - 4_000_000,
            Some("old"),
            200,
            Some(100),
            Some(20),
            Some(999),
        ),
        record(
            now_ms - 3_000_000,
            Some("gpt-a"),
            200,
            Some(30),
            Some(5),
            Some(12),
        ),
        record(now_ms - 1_000_000, Some("gpt-a"), 500, None, None, None),
        record(now_ms - 100_000, Some("gpt-b"), 201, None, Some(7), None),
    ];
    store
        .append_request_logs(&records, MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append logs");

    let overview =
        load_request_log_overview_at(store.pool(), RequestLogOverviewRange::OneHour, now_ms)
            .await
            .expect("overview");

    assert_eq!(overview.retained_started_at_ms, Some(now_ms - 4_000_000));
    assert_eq!(overview.retained_totals.request_count, 4);
    assert_eq!(overview.retained_totals.total_tokens(), 162);
    assert_eq!(overview.retained_totals.cache_read_tokens, 112);
    assert_eq!(overview.retained_totals.token_usage_request_count, 3);
    assert_eq!(overview.range_totals.request_count, 3);
    assert_eq!(overview.range_totals.successful_request_count, 2);
    assert_eq!(overview.range_totals.failed_request_count(), 1);
    assert_eq!(overview.range_totals.input_tokens, 30);
    assert_eq!(overview.range_totals.output_tokens, 12);
    assert_eq!(overview.range_totals.cache_read_tokens, 12);
    assert_eq!(overview.range_totals.token_usage_request_count, 2);
    assert_eq!(overview.time_buckets.len(), 12);
    assert_eq!(
        overview
            .time_buckets
            .iter()
            .map(|bucket| bucket.request_count)
            .sum::<u64>(),
        3
    );
    assert!(
        overview
            .time_buckets
            .iter()
            .any(|bucket| bucket.request_count == 0)
    );
    assert_eq!(overview.models.len(), 2);
    assert_eq!(overview.models[0].public_model.as_deref(), Some("gpt-a"));
    assert_eq!(overview.models[0].totals.request_count, 2);
}

#[tokio::test]
async fn overview_bounds_model_groups_and_distinguishes_unknown_from_other() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("models.sqlite3"))
        .await
        .expect("storage");
    let now_ms = 100_000_000;
    let mut records = vec![record(now_ms - 1, None, 200, Some(1), Some(1), Some(1))];
    records.extend((0..14).map(|index| {
        record(
            now_ms - 10_000 - index,
            Some(&format!("model-{index:02}")),
            if index == 13 { 500 } else { 200 },
            Some(index + 1),
            Some(1),
            Some(index.div_ceil(2)),
        )
    }));
    store
        .append_request_logs(&records, MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append logs");

    let overview = load_request_log_overview_at(
        store.pool(),
        RequestLogOverviewRange::TwentyFourHours,
        now_ms,
    )
    .await
    .expect("overview");

    assert_eq!(overview.models.len(), 13);
    assert!(
        overview
            .models
            .iter()
            .any(|model| model.public_model.is_none() && !model.is_other)
    );
    let other = overview.models.last().expect("other model group");
    assert!(other.is_other);
    assert!(other.public_model.is_none());
    assert_eq!(other.totals.request_count, 3);
    assert_eq!(
        overview
            .models
            .iter()
            .map(|model| model.totals.request_count)
            .sum::<u64>(),
        overview.range_totals.request_count
    );
    assert_eq!(
        overview
            .models
            .iter()
            .map(|model| model.totals.total_tokens())
            .sum::<u128>(),
        overview.range_totals.total_tokens()
    );
    assert_eq!(
        overview
            .models
            .iter()
            .map(|model| model.totals.cache_read_tokens)
            .sum::<u64>(),
        overview.range_totals.cache_read_tokens
    );
}

#[tokio::test]
async fn overview_counts_a_200_stream_error_as_failed() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("stream-outcome.sqlite3"))
        .await
        .expect("storage");
    let now_ms = 10_000_000;
    let success = record(now_ms - 2, Some("gpt-a"), 200, None, None, None);
    let mut failed = record(now_ms - 1, Some("gpt-a"), 200, None, None, None);
    failed.request.error_class = Some(ErrorClass::Upstream);
    store
        .append_request_logs(&[success, failed], MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append logs");

    let overview =
        load_request_log_overview_at(store.pool(), RequestLogOverviewRange::OneHour, now_ms)
            .await
            .expect("overview");

    assert_eq!(overview.range_totals.request_count, 2);
    assert_eq!(overview.range_totals.successful_request_count, 1);
    assert_eq!(overview.range_totals.failed_request_count(), 1);
    assert_eq!(overview.models[0].totals.successful_request_count, 1);
}

fn record(
    started_at_ms: u64,
    public_model: Option<&str>,
    status_code: u16,
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
            public_model: public_model.map(str::to_owned),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: None,
            proxy_profile_id: None,
            status_code,
            error_class: None,
            error_message: None,
            attempt_count: 0,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens: None,
            quota_cost: None,
            is_stream: false,
        },
        attempts: Vec::new(),
        telemetry_position: None,
    }
}

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use any2api_contract_tests::TestApplication;
use any2api_domain::{
    CompletedRequestLog, ConfigRevision, MAX_REQUEST_LOG_ROWS, ProtocolDialect, ProtocolOperation,
    RequestId, RequestLog,
};
use any2api_runtime::api::RequestTelemetry;
use any2api_storage::api::{RequestLogRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header::CACHE_CONTROL},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::Connection;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn overview_usage_exposes_real_tokens_and_bounded_time_and_model_views() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("storage"),
    );
    let now_ms = unix_now_ms();
    storage
        .append_request_logs(
            &[
                record(
                    now_ms - 2 * 60 * 60 * 1_000,
                    Some("old-model"),
                    200,
                    Some(100),
                    Some(20),
                    Some(999),
                ),
                record(
                    now_ms - 20 * 60 * 1_000,
                    Some("gpt-a"),
                    200,
                    Some(30),
                    Some(5),
                    Some(12),
                ),
                record(now_ms - 10 * 60 * 1_000, None, 503, None, None, None),
            ],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append logs");
    let (_directory, app) = test_router(directory, Arc::clone(&storage)).await;

    let (status, headers, body) = request(app.clone(), "/api/admin/overview/usage?range=1h").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(CACHE_CONTROL).expect("no-store"), "no-store");
    assert_eq!(body["range"], "1h");
    assert_eq!(body["retained"]["request_count"], 3);
    assert_eq!(body["retained"]["total_tokens"], "155");
    assert_eq!(body["retained"]["cache_read_tokens"], "112");
    assert_eq!(body["retained"]["token_usage_request_count"], 2);
    assert_eq!(body["selected"]["request_count"], 2);
    assert_eq!(body["selected"]["successful_request_count"], 1);
    assert_eq!(body["selected"]["failed_request_count"], 1);
    assert_eq!(body["selected"]["total_tokens"], "35");
    assert_eq!(body["selected"]["cache_read_tokens"], "12");
    let buckets = body["time_buckets"].as_array().expect("time buckets");
    assert_eq!(buckets.len(), 12);
    assert_eq!(
        buckets
            .iter()
            .map(|bucket| bucket["request_count"].as_u64().expect("count"))
            .sum::<u64>(),
        2
    );
    let models = body["models"].as_array().expect("models");
    assert!(models.iter().any(|model| model["public_model"] == "gpt-a"));
    assert_eq!(
        models
            .iter()
            .find(|model| model["public_model"] == "gpt-a")
            .expect("gpt-a model")["cache_read_tokens"],
        "12"
    );
    assert!(
        models
            .iter()
            .any(|model| model["public_model"].is_null() && model["is_other"] == false)
    );

    let (status, _, body) = request(app.clone(), "/api/admin/overview/usage").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["range"], "24h");
    assert_eq!(body["selected"]["request_count"], 3);
    assert_eq!(body["time_buckets"].as_array().expect("buckets").len(), 24);

    let (status, _, body) = request(app, "/api/admin/overview/usage?range=year").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn overview_resources_exposes_current_process_and_system_load() {
    let fixture = TestApplication::new().await;
    let app = fixture.router();

    let (status, headers, body) = request(app, "/api/admin/overview/resources").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(CACHE_CONTROL).expect("no-store"), "no-store");
    assert!(
        body["sampled_at_ms"]
            .as_u64()
            .is_some_and(|value| value > 0)
    );
    assert!(body["process"]["resident_memory_bytes"].as_u64().is_some());
    assert!(
        body["process"]["cpu_usage_percent"]
            .as_f64()
            .is_some_and(|value| (0.0..=100.0).contains(&value))
    );
    let used = body["system"]["used_memory_bytes"]
        .as_u64()
        .expect("used memory");
    let total = body["system"]["total_memory_bytes"]
        .as_u64()
        .expect("total memory");
    assert!(total > 0);
    assert!(used <= total);
    assert!(
        body["system"]["cpu_usage_percent"]
            .as_f64()
            .is_some_and(|value| (0.0..=100.0).contains(&value))
    );
    let payload_buffers = &body["ownership"]["payload_buffers"];
    for field in [
        "heap_current_bytes",
        "heap_peak_bytes",
        "mapped_current_bytes",
        "mapped_peak_bytes",
        "http_body_capture_current_bytes",
        "http_body_capture_peak_bytes",
    ] {
        assert!(payload_buffers[field].as_u64().is_some(), "missing {field}");
    }
    let telemetry = &body["ownership"]["telemetry"];
    for field in [
        "queued_owned_bytes",
        "in_flight_owned_bytes",
        "reserved_owned_bytes",
    ] {
        assert!(telemetry[field].as_u64().is_some(), "missing {field}");
    }
    let reclamation = &body["ownership"]["reclamation"];
    for field in ["blockers", "completed_runs", "last_duration_micros"] {
        assert!(reclamation[field].as_u64().is_some(), "missing {field}");
    }
}

#[tokio::test]
async fn request_log_list_isolates_corrupt_rows_but_detail_remains_unavailable() {
    let directory = tempdir().expect("temporary directory");
    let database_path = directory.path().join("any2api.sqlite3");
    let storage = Arc::new(SqliteStore::connect(&database_path).await.expect("storage"));
    let now_ms = unix_now_ms();
    let oldest = record(
        now_ms.saturating_sub(3_000),
        Some("old-model"),
        200,
        None,
        None,
        None,
    );
    let corrupt = record(
        now_ms.saturating_sub(2_000),
        Some("bad-model"),
        200,
        None,
        None,
        None,
    );
    let newest = record(
        now_ms.saturating_sub(1_000),
        Some("new-model"),
        200,
        None,
        None,
        None,
    );
    storage
        .append_request_logs(
            &[oldest.clone(), corrupt.clone(), newest.clone()],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append request logs");
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(database_path),
    )
    .await
    .expect("secondary SQLite connection");
    sqlx::query(
        "UPDATE request_logs SET error_message = ' invalid diagnostic' WHERE request_id = ?",
    )
    .bind(corrupt.request.request_id.to_string())
    .execute(&mut connection)
    .await
    .expect("inject corrupt telemetry");

    let (_directory, app) = test_router(directory, Arc::clone(&storage)).await;
    let (status, _, body) = request(app.clone(), "/api/admin/request-logs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("total").is_none());
    let items = body["items"].as_array().expect("request log items");
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0]["request_id"],
        newest.request.request_id.to_string()
    );
    assert_eq!(
        items[1]["request_id"],
        oldest.request.request_id.to_string()
    );

    let (status, _, body) = request(
        app,
        &format!("/api/admin/request-logs/{}", corrupt.request.request_id),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"]["code"], "request_log_unavailable");
}

async fn test_router(
    directory: tempfile::TempDir,
    storage: Arc<SqliteStore>,
) -> (tempfile::TempDir, Router) {
    let fixture = TestApplication::from_storage(directory, storage).await;
    let runtime = fixture.runtime();
    let snapshots = fixture.snapshots();
    let telemetry = Arc::new(RequestTelemetry::start(
        fixture.storage(),
        snapshots.load().revision(),
        snapshots.load().settings().logging(),
        &runtime.lifecycle(),
    ));
    let state = fixture.state().with_request_telemetry(telemetry);
    let (directory, app, _storage) = fixture.into_router_with_state(state);
    (directory, app)
}

async fn request(app: Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41000))))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).expect("json"),
    )
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
            client_ip: "127.0.0.1".parse().expect("loopback address"),
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
            attempt_count: 1,
            latency_ms: 10,
            first_token_ms: None,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens: None,
            quota_cost: None,
            requested_speed_tier: None,
            effective_speed_tier: None,
            is_stream: false,
        },
        attempts: Vec::new(),
        telemetry_position: None,
    }
}

fn unix_now_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_millis(),
    )
    .expect("time fits u64")
}

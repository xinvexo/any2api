use std::{
    fs,
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{
    CompletedRequestLog, ConfigRevision, ProtocolDialect, ProtocolOperation, RequestId, RequestLog,
};
use any2api_runtime::api::{
    ConfigPublisher, PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, RequestLogRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header::CACHE_CONTROL},
};
use http_body_util::BodyExt;
use serde_json::Value;
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
        .append_request_logs(&[
            record(
                now_ms - 2 * 60 * 60 * 1_000,
                Some("old-model"),
                200,
                Some(100),
                Some(20),
            ),
            record(
                now_ms - 20 * 60 * 1_000,
                Some("gpt-a"),
                200,
                Some(30),
                Some(5),
            ),
            record(now_ms - 10 * 60 * 1_000, None, 503, None, None),
        ])
        .await
        .expect("append logs");
    let app = test_router(&directory, Arc::clone(&storage)).await;

    let (status, headers, body) = request(app.clone(), "/api/admin/overview/usage?range=1h").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(CACHE_CONTROL).expect("no-store"), "no-store");
    assert_eq!(body["range"], "1h");
    assert_eq!(body["retained"]["request_count"], 3);
    assert_eq!(body["retained"]["total_tokens"], "155");
    assert_eq!(body["retained"]["token_usage_request_count"], 2);
    assert_eq!(body["selected"]["request_count"], 2);
    assert_eq!(body["selected"]["successful_request_count"], 1);
    assert_eq!(body["selected"]["failed_request_count"], 1);
    assert_eq!(body["selected"]["total_tokens"], "35");
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

async fn test_router(directory: &tempfile::TempDir, storage: Arc<SqliteStore>) -> Router {
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
        any2api_contract_tests::build_provider_registry().as_ref(),
    )));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            any2api_contract_tests::build_configuration_capabilities(),
        )
        .expect("configuration publisher"),
    );
    let telemetry = Arc::new(RequestTelemetry::start(
        storage,
        snapshots.load().revision(),
        snapshots.load().settings().logging(),
        &runtime.lifecycle(),
    ));
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    build_router(
        AppState::new(
            snapshots,
            runtime,
            publisher,
            build_public_request_components()
                .expect("components")
                .service(),
        )
        .with_request_telemetry(telemetry),
        web_root,
    )
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
            cache_read_tokens: Some(999),
            cache_write_tokens: Some(888),
            is_stream: false,
        },
        attempts: Vec::new(),
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

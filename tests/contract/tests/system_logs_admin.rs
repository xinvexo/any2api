use std::{fs, net::SocketAddr, sync::Arc, time::Duration};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{
    ConfigPublisher, PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, HttpAccessLogRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn system_logs_page_auditable_traffic_and_clear_in_writer_order() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("SQLite store"),
    );
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::create_dir(web_root.join("assets")).expect("asset directory");
    fs::write(web_root.join("index.html"), "<main>any2api</main>").expect("web index");

    let (app, telemetry) = build_test_app(Arc::clone(&storage), web_root).await;
    for (uri, status) in [
        ("/api/health?token=must-not-be-stored", StatusCode::OK),
        ("/client/actual%20value?code=secret", StatusCode::OK),
        (
            "/v1/models?api_key=must-not-be-stored",
            StatusCode::UNAUTHORIZED,
        ),
        (
            "/api/not-a-real-route?session=secret",
            StatusCode::NOT_FOUND,
        ),
    ] {
        let response = send(&app, Method::GET, uri).await;
        assert_eq!(response.status, status);
        assert!(response.request_id.len() > 20);
    }
    let unknown_client = send_without_peer(&app, Method::GET, "/api/health").await;
    assert_eq!(unknown_client.status, StatusCode::OK);

    wait_for_log_count(storage.as_ref(), 3).await;
    let (mut log_events, initial_events) = open_log_events(&app).await;
    assert!(initial_events.contains("event: request_logs_changed\n"));
    assert!(initial_events.contains("event: system_logs_changed\n"));
    assert_eq!(
        storage
            .list_http_access_logs(0, 0, 100)
            .await
            .expect("HTTP access logs after SSE connect")
            .total,
        3
    );

    let changed = send(&app, Method::GET, "/api/another-missing-route").await;
    assert_eq!(changed.status, StatusCode::NOT_FOUND);
    let changed_event = next_sse_data(&mut log_events).await;
    assert!(changed_event.contains("event: system_logs_changed\n"));
    wait_for_log_count(storage.as_ref(), 4).await;
    drop(log_events);
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        storage
            .list_http_access_logs(0, 0, 100)
            .await
            .expect("HTTP access logs after SSE disconnect")
            .total,
        4
    );

    let invalid_list = send(&app, Method::GET, "/api/admin/system-logs?page=0").await;
    assert_eq!(invalid_list.status, StatusCode::BAD_REQUEST);
    wait_for_log_count(storage.as_ref(), 5).await;

    let first_page = send(
        &app,
        Method::GET,
        "/api/admin/system-logs?page=1&page_size=3",
    )
    .await;
    assert_eq!(first_page.status, StatusCode::OK);
    assert_eq!(first_page.json["total"], 5);
    assert_eq!(first_page.json["page"], 1);
    assert_eq!(first_page.json["page_size"], 3);
    let second_page = send(
        &app,
        Method::GET,
        "/api/admin/system-logs?page=2&page_size=3",
    )
    .await;
    assert_eq!(second_page.status, StatusCode::OK);
    let items = first_page.json["items"]
        .as_array()
        .expect("first system log page")
        .iter()
        .chain(
            second_page.json["items"]
                .as_array()
                .expect("second system log page"),
        )
        .collect::<Vec<_>>();
    let paths = items
        .iter()
        .map(|item| item["path"].as_str().expect("system log path"))
        .collect::<Vec<_>>();
    assert!(paths.contains(&"/v1/models"));
    assert!(paths.contains(&"/api/not-a-real-route"));
    assert!(paths.contains(&"/api/health"));
    assert!(paths.contains(&"/api/admin/system-logs"));
    assert!(!paths.contains(&"/client/actual%20value"));
    assert!(paths.iter().all(|path| !path.contains('?')));
    let health = items
        .iter()
        .find(|item| item["path"] == "/api/health" && item["client_ip"].is_null())
        .expect("unknown-client health access log");
    assert_eq!(health["status_code"], 200);
    assert_eq!(health["outcome"], "completed");
    assert!(health["response_bytes"].as_u64().expect("response bytes") > 0);
    let unauthorized = items
        .iter()
        .find(|item| item["path"] == "/v1/models")
        .expect("public authentication failure access log");
    assert_eq!(unauthorized["status_code"], 401);

    let cleared = send(&app, Method::DELETE, "/api/admin/system-logs").await;
    assert_eq!(cleared.status, StatusCode::OK);
    assert_eq!(cleared.json["deleted"], 5);

    tokio::time::sleep(Duration::from_millis(20)).await;
    let remaining = storage
        .list_http_access_logs(0, 0, 100)
        .await
        .expect("remaining HTTP access logs");
    assert_eq!(remaining.total, 0);
    assert!(remaining.items.is_empty());

    let denied = send_from_peer(
        &app,
        Method::GET,
        "/api/admin/log-events",
        SocketAddr::from(([203, 0, 113, 9], 41_001)),
    )
    .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(denied.json["error"]["code"], "admin_loopback_only");
    wait_for_log_count(storage.as_ref(), 1).await;
    let denied_logs = storage
        .list_http_access_logs(0, 0, 100)
        .await
        .expect("denied SSE access log");
    assert_eq!(denied_logs.items[0].path, "/api/admin/log-events");

    telemetry.shutdown(Duration::from_secs(1)).await;
}

async fn build_test_app(
    storage: Arc<SqliteStore>,
    web_root: std::path::PathBuf,
) -> (Router, Arc<RequestTelemetry>) {
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        configuration.revision(),
        configuration.settings().logging(),
        &runtime.lifecycle(),
    ));
    let components = build_public_request_components().expect("public request components");
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
        components.provider_registry(),
    )));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            components.configuration_capabilities(),
        )
        .expect("configuration publisher"),
    );
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, components.service())
            .with_request_telemetry(Arc::clone(&telemetry)),
        web_root,
    );
    (app, telemetry)
}

struct TestResponse {
    status: StatusCode,
    request_id: String,
    json: serde_json::Value,
}

async fn send(app: &Router, method: Method, uri: &str) -> TestResponse {
    send_request(app, method, uri).await
}

async fn send_without_peer(app: &Router, method: Method, uri: &str) -> TestResponse {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    collect_response(response).await
}

async fn send_request(app: &Router, method: Method, uri: &str) -> TestResponse {
    send_from_peer(app, method, uri, SocketAddr::from(([127, 0, 0, 1], 41_000))).await
}

async fn send_from_peer(app: &Router, method: Method, uri: &str, peer: SocketAddr) -> TestResponse {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(peer));
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    collect_response(response).await
}

async fn open_log_events(app: &Router) -> (Body, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/admin/log-events")
                .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41_000))))
                .body(Body::empty())
                .expect("log events request"),
        )
        .await
        .expect("log events response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let mut body = response.into_body();
    let first = next_sse_data(&mut body).await;
    let second = next_sse_data(&mut body).await;
    (body, first + &second)
}

async fn next_sse_data(body: &mut Body) -> String {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(1), body.frame())
            .await
            .expect("SSE frame timeout")
            .expect("SSE stream remains open")
            .expect("SSE frame");
        if let Ok(data) = frame.into_data() {
            return String::from_utf8(data.to_vec()).expect("UTF-8 SSE event");
        }
    }
}

async fn collect_response(response: axum::response::Response) -> TestResponse {
    let status = response.status();
    let request_id = response.headers()["x-request-id"]
        .to_str()
        .expect("request ID header")
        .to_owned();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    TestResponse {
        status,
        request_id,
        json,
    }
}

async fn wait_for_log_count(storage: &SqliteStore, minimum: usize) {
    for _ in 0..200 {
        if storage
            .list_http_access_logs(0, 0, 100)
            .await
            .expect("HTTP access logs")
            .items
            .len()
            >= minimum
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("HTTP access logs were not persisted");
}

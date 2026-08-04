use std::{net::SocketAddr, sync::Arc, time::Duration};

use any2api_contract_tests::TestApplication;
use any2api_domain::RequestId;
use any2api_runtime::api::RequestTelemetry;
use any2api_storage::api::{HttpAccessLogRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn system_logs_page_auditable_traffic_and_clear_in_writer_order() {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let (_directory, app, telemetry) = build_test_app(fixture).await;
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
            .list_http_access_logs(0, None, 100)
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
    let invalid_list = send(
        &app,
        Method::GET,
        "/api/admin/system-logs?cursor=not-a-cursor",
    )
    .await;
    assert_eq!(invalid_list.status, StatusCode::BAD_REQUEST);
    wait_for_log_count(storage.as_ref(), 5).await;
    drop(log_events);

    let first_page = send(&app, Method::GET, "/api/admin/system-logs?page_size=3").await;
    assert_eq!(first_page.status, StatusCode::OK);
    assert_eq!(first_page.json["total"], 5);
    assert_eq!(first_page.json["page_size"], 3);
    let first_cursor = first_page.json["cursor"]
        .as_str()
        .expect("first system log cursor");
    let next_cursor = first_page.json["next_cursor"]
        .as_str()
        .expect("next system log cursor");
    assert_ne!(first_cursor, next_cursor);
    let request_cursor = first_cursor.replacen("s1.", "r1.", 1);
    let wrong_system_cursor = send(
        &app,
        Method::GET,
        &format!("/api/admin/system-logs?cursor={request_cursor}"),
    )
    .await;
    assert_eq!(wrong_system_cursor.status, StatusCode::BAD_REQUEST);
    let wrong_request_cursor = send(
        &app,
        Method::GET,
        &format!("/api/admin/request-logs?cursor={first_cursor}"),
    )
    .await;
    assert_eq!(wrong_request_cursor.status, StatusCode::BAD_REQUEST);
    wait_for_log_count(storage.as_ref(), 7).await;
    assert!(first_page.json["telemetry"]["queued_records"].is_u64());
    assert!(first_page.json["telemetry"]["in_flight_records"].is_u64());
    assert!(first_page.json["telemetry"]["dropped_records"].is_u64());
    assert!(first_page.json["telemetry"]["persisted_records"].is_u64());
    let second_page = send(
        &app,
        Method::GET,
        &format!("/api/admin/system-logs?page_size=3&cursor={next_cursor}"),
    )
    .await;
    assert_eq!(second_page.status, StatusCode::OK);
    assert_eq!(second_page.json["cursor"], next_cursor);
    assert!(second_page.json["next_cursor"].is_null());
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
    let unauthorized_id = unauthorized["request_id"]
        .as_str()
        .expect("public authentication failure request ID")
        .parse::<RequestId>()
        .expect("valid request ID");
    assert!(
        storage
            .get_http_access_log(unauthorized_id)
            .await
            .expect("public authentication failure detail")
            .expect("public authentication failure access log")
            .gateway_auth_rejected
    );

    let cleared = send(&app, Method::DELETE, "/api/admin/system-logs").await;
    assert_eq!(cleared.status, StatusCode::OK);
    assert_eq!(cleared.json["deleted"], 7);

    tokio::time::sleep(Duration::from_millis(20)).await;
    let remaining = storage
        .list_http_access_logs(0, None, 100)
        .await
        .expect("remaining HTTP access logs");
    assert_eq!(remaining.total, 0);
    assert!(remaining.items.is_empty());

    let mut system_log_changes = telemetry.subscribe_http_access_log_changes();
    let epoch_before_denied = *system_log_changes.borrow_and_update();
    let denied = send_from_peer(
        &app,
        Method::GET,
        "/api/admin/log-events",
        SocketAddr::from(([203, 0, 113, 9], 41_001)),
    )
    .await;
    assert_eq!(denied.status, StatusCode::UNAUTHORIZED);
    assert_eq!(denied.json["error"]["code"], "admin_session_required");
    wait_for_log_count(storage.as_ref(), 1).await;
    tokio::time::timeout(Duration::from_secs(1), system_log_changes.changed())
        .await
        .expect("denied request change notification")
        .expect("system log notifier remains open");
    let epoch_after_denied = *system_log_changes.borrow_and_update();
    assert!(epoch_after_denied > epoch_before_denied);
    let denied_logs = storage
        .list_http_access_logs(0, None, 100)
        .await
        .expect("denied SSE access log");
    assert_eq!(denied_logs.items[0].path, "/api/admin/log-events");

    let self_read = send(
        &app,
        Method::GET,
        "/api/admin/system-logs?cursor=still-not-a-cursor",
    )
    .await;
    assert_eq!(self_read.status, StatusCode::BAD_REQUEST);
    wait_for_log_count(storage.as_ref(), 2).await;
    telemetry.shutdown(Duration::from_secs(1)).await;
    assert_eq!(*system_log_changes.borrow(), epoch_after_denied);
}

#[tokio::test]
async fn ipv4_mapped_loopback_uses_canonical_system_log_retention_semantics() {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let (_directory, app, telemetry) = build_test_app(fixture).await;
    let mapped_loopback = "[::ffff:127.0.0.1]:41000"
        .parse::<SocketAddr>()
        .expect("mapped loopback peer");

    let health = send_from_peer(&app, Method::GET, "/api/health", mapped_loopback).await;
    assert_eq!(health.status, StatusCode::OK);
    let public = send_from_peer(&app, Method::GET, "/v1/models", mapped_loopback).await;
    assert_eq!(public.status, StatusCode::UNAUTHORIZED);
    wait_for_log_count(storage.as_ref(), 1).await;

    let logs = storage
        .list_http_access_logs(0, None, 10)
        .await
        .expect("mapped loopback system logs");
    assert_eq!(logs.total, 1);
    assert_eq!(logs.items[0].path, "/v1/models");
    assert_eq!(
        logs.items[0].client_ip,
        Some("127.0.0.1".parse().expect("canonical loopback"))
    );
    telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn system_log_detail_preserves_raw_client_http_exchange() {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let (_directory, app, telemetry) = build_test_app(fixture).await;
    let raw_body = r#"{"password":"raw-password","keyword":"do-not-redact"}"#;
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/admin/auth/login?search=raw-query-value")
        .header("content-type", "application/json")
        .header("authorization", "Bearer raw-gateway-key")
        .header("x-repeated", "first")
        .header("x-repeated", "second")
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41_000))))
        .body(Body::from(raw_body))
        .expect("raw request");
    let response = app.clone().oneshot(request).await.expect("raw response");
    let response = collect_response(response).await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    let request_id = response
        .request_id
        .parse::<RequestId>()
        .expect("system log request ID");
    wait_for_log_detail(storage.as_ref(), request_id).await;

    let detail = send(
        &app,
        Method::GET,
        &format!("/api/admin/system-logs/{request_id}"),
    )
    .await;
    assert_eq!(detail.status, StatusCode::OK);
    assert_eq!(
        detail.json["log"]["uri"],
        "/api/admin/auth/login?search=raw-query-value"
    );
    assert_eq!(detail.json["log"]["exchange_captured"], true);
    let request_headers = detail.json["exchange"]["request"]["headers"]
        .as_array()
        .expect("request headers");
    let repeated = request_headers
        .iter()
        .filter(|header| header["name"] == "x-repeated")
        .map(|header| header["value"].as_str().expect("header value"))
        .collect::<Vec<_>>();
    assert_eq!(repeated, vec!["first", "second"]);
    assert!(request_headers.iter().any(|header| {
        header["name"] == "authorization" && header["value"] == "Bearer raw-gateway-key"
    }));
    assert_eq!(
        detail.json["exchange"]["request"]["body"]["content"],
        raw_body
    );
    assert_eq!(
        detail.json["exchange"]["request"]["body"]["total_bytes"],
        raw_body.len()
    );
    assert_eq!(detail.json["exchange"]["request"]["body"]["complete"], true);
    assert_eq!(
        detail.json["exchange"]["request"]["body"]["truncated"],
        false
    );
    let response_headers = detail.json["exchange"]["response"]["headers"]
        .as_array()
        .expect("response headers");
    assert!(response_headers.iter().any(|header| {
        header["name"] == "x-any2api-request-id" && header["value"] == request_id.to_string()
    }));
    assert!(
        detail.json["exchange"]["response"]["body"]["content"]
            .as_str()
            .expect("response body")
            .contains("admin_invalid_credentials")
    );
    assert_eq!(
        detail.json["exchange"]["response"]["body"]["complete"],
        true
    );

    telemetry.shutdown(Duration::from_secs(1)).await;
}

async fn build_test_app(
    fixture: TestApplication,
) -> (tempfile::TempDir, Router, Arc<RequestTelemetry>) {
    let storage = fixture.storage();
    let runtime = fixture.runtime();
    let snapshots = fixture.snapshots();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        snapshots.load().revision(),
        snapshots.load().settings().logging(),
        &runtime.lifecycle(),
    ));
    let state = fixture
        .state()
        .with_request_telemetry(Arc::clone(&telemetry));
    let (directory, app, _storage) = fixture.into_router_with_state(state);
    (directory, app, telemetry)
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
            .list_http_access_logs(0, None, 100)
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

async fn wait_for_log_detail(storage: &SqliteStore, request_id: RequestId) {
    for _ in 0..200 {
        if storage
            .get_http_access_log(request_id)
            .await
            .expect("HTTP access log detail")
            .is_some()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("HTTP access log detail was not persisted");
}

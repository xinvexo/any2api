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
async fn complete_http_logs_keep_exact_paths_and_clear_in_writer_order() {
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
        (
            "/v1/models?api_key=must-not-be-stored",
            StatusCode::UNAUTHORIZED,
        ),
        ("/client/actual%20value?code=secret", StatusCode::OK),
        (
            "/api/not-a-real-route?session=secret",
            StatusCode::NOT_FOUND,
        ),
    ] {
        let response = send(&app, Method::GET, uri).await;
        assert_eq!(response.status, status);
        assert!(response.request_id.len() > 20);
    }

    wait_for_log_count(storage.as_ref(), 4).await;
    let automatic_list =
        send_automatic_refresh(&app, Method::GET, "/api/admin/system-logs?limit=100").await;
    assert_eq!(automatic_list.status, StatusCode::OK);

    let invalid_automatic_list =
        send_automatic_refresh(&app, Method::GET, "/api/admin/system-logs?limit=0").await;
    assert_eq!(invalid_automatic_list.status, StatusCode::BAD_REQUEST);

    let list = send(&app, Method::GET, "/api/admin/system-logs?limit=100").await;
    assert_eq!(list.status, StatusCode::OK);
    let items = list.json["items"].as_array().expect("system log items");
    let paths = items
        .iter()
        .map(|item| item["path"].as_str().expect("system log path"))
        .collect::<Vec<_>>();
    assert!(paths.contains(&"/api/health"));
    assert!(paths.contains(&"/v1/models"));
    assert!(paths.contains(&"/client/actual%20value"));
    assert!(paths.contains(&"/api/not-a-real-route"));
    assert!(paths.iter().all(|path| !path.contains('?')));
    assert!(items.iter().all(|item| item["client_ip"] == "127.0.0.1"));
    let health = items
        .iter()
        .find(|item| item["path"] == "/api/health")
        .expect("health access log");
    assert_eq!(health["status_code"], 200);
    assert_eq!(health["outcome"], "completed");
    assert!(health["response_bytes"].as_u64().expect("response bytes") > 0);
    let unauthorized = items
        .iter()
        .find(|item| item["path"] == "/v1/models")
        .expect("public authentication failure access log");
    assert_eq!(unauthorized["status_code"], 401);

    wait_for_log_count(storage.as_ref(), 6).await;
    let persisted = storage
        .list_http_access_logs(100)
        .await
        .expect("persisted HTTP access logs");
    let system_log_reads = persisted
        .iter()
        .filter(|log| log.method == "GET" && log.path == "/api/admin/system-logs")
        .collect::<Vec<_>>();
    assert_eq!(system_log_reads.len(), 2);
    assert!(
        system_log_reads
            .iter()
            .any(|log| log.status_code == Some(200))
    );
    assert!(
        system_log_reads
            .iter()
            .any(|log| log.status_code == Some(400))
    );

    let cleared = send(&app, Method::DELETE, "/api/admin/system-logs").await;
    assert_eq!(cleared.status, StatusCode::OK);
    assert!(cleared.json["deleted"].as_u64().expect("deleted count") >= 4);

    wait_for_delete_audit(storage.as_ref()).await;
    let remaining = storage
        .list_http_access_logs(100)
        .await
        .expect("remaining HTTP access logs");
    assert!(
        remaining
            .iter()
            .all(|log| log.path == "/api/admin/system-logs")
    );
    assert!(remaining.iter().any(|log| log.method == "DELETE"));

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
    send_request(app, method, uri, false).await
}

async fn send_automatic_refresh(app: &Router, method: Method, uri: &str) -> TestResponse {
    send_request(app, method, uri, true).await
}

async fn send_request(
    app: &Router,
    method: Method,
    uri: &str,
    automatic_refresh: bool,
) -> TestResponse {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41_000))));
    if automatic_refresh {
        request = request.header("x-any2api-system-log-refresh", "automatic");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
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
            .list_http_access_logs(100)
            .await
            .expect("HTTP access logs")
            .len()
            >= minimum
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("HTTP access logs were not persisted");
}

async fn wait_for_delete_audit(storage: &SqliteStore) {
    for _ in 0..200 {
        if storage
            .list_http_access_logs(100)
            .await
            .expect("HTTP access logs")
            .iter()
            .any(|log| log.method == "DELETE")
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("system log clear audit record was not persisted");
}

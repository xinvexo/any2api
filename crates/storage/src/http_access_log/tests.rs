use any2api_domain::{
    ConfigRevision, HttpAccessLog, HttpAccessLogOutcome, HttpProtocolVersion, RequestId,
};
use tempfile::tempdir;

use crate::{http_access_log::HttpAccessLogRepository, sqlite::SqliteStore};

#[tokio::test]
async fn stores_exact_paths_prunes_and_clears_history() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log.sqlite3"))
        .await
        .expect("SQLite store");
    let first = record(100, "/api/admin/provider-credentials/actual-id");
    let second = record(200, "/assets/app%20name.js");

    store
        .append_http_access_logs(&[first.clone(), second.clone()])
        .await
        .expect("append access logs");
    assert_eq!(
        store
            .list_http_access_logs(0, 0, 10)
            .await
            .expect("list access logs"),
        any2api_domain::LogPage::new(vec![second.clone(), first], 2)
    );

    assert_eq!(
        store
            .prune_http_access_logs(150, 10, 100)
            .await
            .expect("prune access logs"),
        1
    );
    assert_eq!(
        store
            .clear_http_access_logs()
            .await
            .expect("clear access logs"),
        1
    );
    assert!(
        store
            .list_http_access_logs(0, 0, 10)
            .await
            .expect("empty access logs")
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn access_log_pages_hide_local_success_noise_and_keep_auditable_traffic() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-pages.sqlite3"))
        .await
        .expect("SQLite store");
    let mut local_noise = record(200, "/favicon.ico");
    local_noise.client_ip = Some("127.0.0.1".parse().expect("loopback"));
    let mut local_proxy = record(300, "/v1/responses");
    local_proxy.client_ip = Some("127.0.0.1".parse().expect("loopback"));
    let mut local_uppercase_noise = record(350, "/V1/responses");
    local_uppercase_noise.client_ip = Some("127.0.0.1".parse().expect("loopback"));
    let mut local_error = record(400, "/api/health");
    local_error.client_ip = Some("::1".parse().expect("loopback"));
    local_error.status_code = Some(500);
    let external = record(500, "/api/admin/settings");
    store
        .append_http_access_logs(&[
            local_noise,
            local_proxy,
            local_uppercase_noise,
            local_error.clone(),
            external,
        ])
        .await
        .expect("append access logs");

    let first_page = store
        .list_http_access_logs(250, 0, 2)
        .await
        .expect("first page");
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.items[1], local_error);

    let second_page = store
        .list_http_access_logs(250, 2, 2)
        .await
        .expect("second page");
    assert_eq!(second_page.total, 3);
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].path, "/v1/responses");
}

fn record(started_at_ms: u64, path: &str) -> HttpAccessLog {
    HttpAccessLog {
        request_id: RequestId::new(),
        started_at_ms,
        config_revision: ConfigRevision::INITIAL,
        client_ip: Some("203.0.113.8".parse().expect("client IP")),
        method: "GET".to_owned(),
        path: path.to_owned(),
        http_version: HttpProtocolVersion::Http11,
        status_code: Some(200),
        duration_ms: 12,
        response_bytes: 42,
        outcome: HttpAccessLogOutcome::Completed,
    }
}

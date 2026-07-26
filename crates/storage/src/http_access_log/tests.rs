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
            .list_http_access_logs(10)
            .await
            .expect("list access logs"),
        vec![second.clone(), first]
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
            .list_http_access_logs(10)
            .await
            .expect("empty access logs")
            .is_empty()
    );
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

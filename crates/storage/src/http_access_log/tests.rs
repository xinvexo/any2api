use any2api_domain::{
    ConfigRevision, HttpAccessLog, HttpAccessLogExchange, HttpAccessLogOutcome, HttpBodyCapture,
    HttpHeader, HttpProtocolVersion, RequestId,
};
use tempfile::tempdir;

use crate::{
    http_access_log::{HttpAccessLogCapacity, HttpAccessLogRepository},
    sqlite::SqliteStore,
};

const GENEROUS_CAPACITY: HttpAccessLogCapacity =
    HttpAccessLogCapacity::new(10_000, 64 * 1024 * 1024);

#[tokio::test]
async fn stores_exact_paths_prunes_and_clears_history() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log.sqlite3"))
        .await
        .expect("SQLite store");
    let first = record(100, "/api/admin/provider-credentials/actual-id");
    let second = record(200, "/assets/app%20name.js");

    store
        .append_http_access_logs(&[first.clone(), second.clone()], GENEROUS_CAPACITY)
        .await
        .expect("append access logs");
    assert_eq!(
        store
            .list_http_access_logs(0, 0, 10)
            .await
            .expect("list access logs"),
        any2api_domain::LogPage::new(vec![second.summary(), first.summary()], 2)
    );
    assert_eq!(
        store
            .get_http_access_log(second.request_id)
            .await
            .expect("access log detail"),
        Some(second.clone())
    );

    assert_eq!(
        store
            .prune_http_access_logs(150, GENEROUS_CAPACITY, 100)
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
    let mut mapped_local_noise = record(375, "/api/health");
    mapped_local_noise.client_ip = Some("::ffff:127.0.0.1".parse().expect("mapped loopback"));
    let mut local_error = record(400, "/api/health");
    local_error.client_ip = Some("::1".parse().expect("loopback"));
    local_error.status_code = Some(500);
    let external = record(500, "/api/admin/settings");
    store
        .append_http_access_logs(
            &[
                local_noise,
                local_proxy,
                local_uppercase_noise,
                mapped_local_noise.clone(),
                local_error.clone(),
                external,
            ],
            GENEROUS_CAPACITY,
        )
        .await
        .expect("append access logs");
    let stored_mapped_ip: String =
        sqlx::query_scalar("SELECT client_ip FROM http_access_logs WHERE request_id = ?")
            .bind(mapped_local_noise.request_id.to_string())
            .fetch_one(store.pool())
            .await
            .expect("stored mapped client IP");
    assert_eq!(stored_mapped_ip, "127.0.0.1");

    let first_page = store
        .list_http_access_logs(250, 0, 2)
        .await
        .expect("first page");
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.items[1], local_error.summary());

    let second_page = store
        .list_http_access_logs(250, 2, 2)
        .await
        .expect("second page");
    assert_eq!(second_page.total, 3);
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].path, "/v1/responses");
}

#[tokio::test]
async fn append_enforces_independent_row_and_exchange_capacity() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-capacity.sqlite3"))
        .await
        .expect("SQLite store");
    let first = sized_record(100, 6);
    let second = sized_record(200, 6);
    let third = sized_record(300, 6);

    assert_eq!(
        store
            .append_http_access_logs(
                &[first, second.clone(), third.clone()],
                HttpAccessLogCapacity::new(2, 100),
            )
            .await
            .expect("row capacity"),
        1
    );
    assert_eq!(
        stored_ids(&store).await,
        vec![second.request_id, third.request_id]
    );

    let fourth = sized_record(400, 16);
    assert_eq!(
        store
            .append_http_access_logs(
                std::slice::from_ref(&fourth),
                HttpAccessLogCapacity::new(10, 25),
            )
            .await
            .expect("exchange capacity"),
        2
    );
    assert_eq!(stored_ids(&store).await, vec![fourth.request_id]);

    let oversized = sized_record(500, 26);
    assert_eq!(
        store
            .append_http_access_logs(
                std::slice::from_ref(&oversized),
                HttpAccessLogCapacity::new(10, 25),
            )
            .await
            .expect("oversized exchange"),
        2
    );
    assert!(stored_ids(&store).await.is_empty());
}

#[tokio::test]
async fn incremental_vacuum_reclaims_pages_after_clear_on_new_databases() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-vacuum.sqlite3"))
        .await
        .expect("SQLite store");
    let large = sized_record(100, 512 * 1024);
    store
        .append_http_access_logs(std::slice::from_ref(&large), GENEROUS_CAPACITY)
        .await
        .expect("large access log");
    store
        .clear_http_access_logs()
        .await
        .expect("clear large access log");
    let pages_before: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(store.pool())
        .await
        .expect("page count before reclaim");
    let freelist_before: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(store.pool())
        .await
        .expect("freelist before reclaim");
    assert!(freelist_before > 0);
    let auto_vacuum: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
        .fetch_one(store.pool())
        .await
        .expect("auto vacuum mode");
    assert_eq!(auto_vacuum, 2);

    let reclaimed = store
        .reclaim_http_access_log_storage(16 * 1024 * 1024)
        .await
        .expect("incremental vacuum");
    let pages_after: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(store.pool())
        .await
        .expect("page count after reclaim");
    let freelist_after: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(store.pool())
        .await
        .expect("freelist after reclaim");
    assert!(
        reclaimed > 0,
        "pages {pages_before}->{pages_after}, freelist {freelist_before}->{freelist_after}"
    );
    assert!(pages_after < pages_before);
}

async fn stored_ids(store: &SqliteStore) -> Vec<RequestId> {
    sqlx::query_scalar::<_, String>(
        "SELECT request_id FROM http_access_logs ORDER BY started_at_ms, request_id",
    )
    .fetch_all(store.pool())
    .await
    .expect("stored access log ids")
    .into_iter()
    .map(|value| value.parse().expect("request id"))
    .collect()
}

fn sized_record(started_at_ms: u64, request_body_bytes: usize) -> HttpAccessLog {
    let mut value = record(started_at_ms, "/v1/responses");
    let exchange = value.exchange.as_mut().expect("captured exchange");
    exchange.request_headers.clear();
    exchange.response_headers.clear();
    exchange.request_body.content = vec![b'r'; request_body_bytes];
    exchange.request_body.total_bytes =
        u64::try_from(request_body_bytes).expect("request body length fits u64");
    exchange.response_body.content.clear();
    exchange.response_body.total_bytes = 0;
    value
}

fn record(started_at_ms: u64, path: &str) -> HttpAccessLog {
    HttpAccessLog {
        request_id: RequestId::new(),
        started_at_ms,
        config_revision: ConfigRevision::INITIAL,
        client_ip: Some("203.0.113.8".parse().expect("client IP")),
        method: "GET".to_owned(),
        path: path.to_owned(),
        uri: format!("{path}?raw=query"),
        http_version: HttpProtocolVersion::Http11,
        status_code: Some(200),
        duration_ms: 12,
        response_bytes: 42,
        outcome: HttpAccessLogOutcome::Completed,
        exchange: Some(HttpAccessLogExchange {
            request_headers: vec![HttpHeader {
                name: "x-raw".to_owned(),
                value: vec![0xff, 0x00, b'a'],
            }],
            request_body: HttpBodyCapture {
                content: b"request body".to_vec(),
                total_bytes: 12,
                complete: true,
                truncated: false,
            },
            response_headers: vec![HttpHeader {
                name: "set-cookie".to_owned(),
                value: b"session=raw".to_vec(),
            }],
            response_body: HttpBodyCapture {
                content: b"response body prefix".to_vec(),
                total_bytes: 42,
                complete: true,
                truncated: true,
            },
        }),
    }
}

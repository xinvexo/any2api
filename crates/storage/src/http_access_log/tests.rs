use any2api_domain::{
    ConfigRevision, HttpAccessLog, HttpAccessLogExchange, HttpAccessLogOutcome, HttpBodyCapture,
    HttpHeader, HttpProtocolVersion, RequestId,
};
use tempfile::tempdir;

use crate::{
    http_access_log::{HttpAccessLogCapacity, HttpAccessLogRepository},
    sqlite::SqliteStore,
};

mod capacity;

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
    let first_summary = first.summary();
    let second_summary = second.summary();
    let second_id = second.request_id;

    store
        .append_http_access_logs(vec![first, second], GENEROUS_CAPACITY)
        .await
        .expect("append access logs");
    let page = store
        .list_http_access_logs(0, true, None, 1, 10)
        .await
        .expect("list access logs");
    assert_eq!(page.items, vec![second_summary.clone(), first_summary]);
    assert_eq!(page.total, 2);
    assert!(page.cursor.is_some());
    assert!(page.next_cursor.is_none());
    let detail = store
        .get_http_access_log(second_id)
        .await
        .expect("access log detail")
        .expect("stored detail");
    assert_eq!(detail.summary(), second_summary);
    assert_eq!(
        detail
            .exchange
            .as_ref()
            .expect("captured exchange")
            .request_body
            .content(),
        b"request body"
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
            .list_http_access_logs(0, true, None, 1, 10)
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
    let mapped_local_id = mapped_local_noise.request_id;
    let local_error_summary = local_error.summary();
    store
        .append_http_access_logs(
            vec![
                local_noise,
                local_proxy,
                local_uppercase_noise,
                mapped_local_noise,
                local_error,
                external,
            ],
            GENEROUS_CAPACITY,
        )
        .await
        .expect("append access logs");
    let stored_mapped_ip: String =
        sqlx::query_scalar("SELECT client_ip FROM http_access_logs WHERE request_id = ?")
            .bind(mapped_local_id.to_string())
            .fetch_one(store.pool())
            .await
            .expect("stored mapped client IP");
    assert_eq!(stored_mapped_ip, "127.0.0.1");

    let first_page = store
        .list_http_access_logs(250, true, None, 1, 2)
        .await
        .expect("first page");
    assert_eq!(first_page.total, 3);
    assert_eq!(first_page.items.len(), 2);
    assert_eq!(first_page.items[1], local_error_summary);

    let second_page = store
        .list_http_access_logs(250, true, first_page.next_cursor, 2, 2)
        .await
        .expect("second page");
    assert_eq!(second_page.total, 3);
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].path, "/v1/responses");
}

#[tokio::test]
async fn access_log_pages_can_hide_only_admin_operation_paths() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-admin-filter.sqlite3"))
        .await
        .expect("SQLite store");
    store
        .append_http_access_logs(
            vec![
                record(100, "/api/admin"),
                record(200, "/api/admin/settings"),
                record(300, "/api/administrator"),
                record(400, "/v1/responses"),
            ],
            GENEROUS_CAPACITY,
        )
        .await
        .expect("append access logs");

    let visible = store
        .list_http_access_logs(0, true, None, 1, 10)
        .await
        .expect("unfiltered page");
    assert_eq!(visible.total, 4);

    let first_page = store
        .list_http_access_logs(0, false, None, 1, 1)
        .await
        .expect("first filtered page");
    assert_eq!(first_page.total, 2);
    assert_eq!(first_page.items[0].path, "/v1/responses");

    let second_page = store
        .list_http_access_logs(0, false, first_page.next_cursor, 2, 1)
        .await
        .expect("second filtered page");
    assert_eq!(second_page.total, 2);
    assert_eq!(second_page.items[0].path, "/api/administrator");
    assert!(second_page.next_cursor.is_none());
}

#[tokio::test]
async fn access_logs_jump_to_an_anchored_page_and_clamp_past_the_end() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-jump.sqlite3"))
        .await
        .expect("SQLite store");
    let records = (1..=7)
        .map(|index| record(index * 100, &format!("/v1/{index}")))
        .collect::<Vec<_>>();
    let expected_ids = records
        .iter()
        .map(|record| record.request_id)
        .collect::<Vec<_>>();
    store
        .append_http_access_logs(records, GENEROUS_CAPACITY)
        .await
        .expect("append access logs");

    let first_page = store
        .list_http_access_logs(0, true, None, 1, 2)
        .await
        .expect("first page");
    let anchor = first_page.cursor.clone();
    let third_page = store
        .list_http_access_logs(0, true, anchor.clone(), 3, 2)
        .await
        .expect("third page");
    assert_eq!(third_page.page, 3);
    assert_eq!(
        third_page
            .items
            .iter()
            .map(|item| item.request_id)
            .collect::<Vec<_>>(),
        [expected_ids[2], expected_ids[1]]
    );

    let last_page = store
        .list_http_access_logs(0, true, anchor, 99, 2)
        .await
        .expect("clamped last page");
    assert_eq!(last_page.page, 4);
    assert_eq!(last_page.items[0].request_id, expected_ids[0]);
    assert!(last_page.next_cursor.is_none());
}

#[tokio::test]
async fn access_log_cursor_is_stable_across_inserts_and_tail_deletion() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-cursor.sqlite3"))
        .await
        .expect("SQLite store");
    let oldest = record(100, "/v1/oldest");
    let older = record(200, "/v1/older");
    let middle = record(300, "/v1/middle");
    let fourth = record(400, "/v1/fourth");
    let newest = record(500, "/v1/newest");
    let oldest_id = oldest.request_id;
    let older_id = older.request_id;
    let middle_id = middle.request_id;
    let fourth_id = fourth.request_id;
    let newest_id = newest.request_id;
    store
        .append_http_access_logs(
            vec![oldest, older, middle, fourth, newest],
            GENEROUS_CAPACITY,
        )
        .await
        .expect("append initial logs");

    let first_page = store
        .list_http_access_logs(0, true, None, 1, 2)
        .await
        .expect("first page");
    assert_eq!(
        first_page
            .items
            .iter()
            .map(|item| item.request_id)
            .collect::<Vec<_>>(),
        [newest_id, fourth_id]
    );

    let head = record(600, "/v1/head");
    let late = record(350, "/v1/late");
    let head_id = head.request_id;
    let late_id = late.request_id;
    store
        .append_http_access_logs(vec![head, late], GENEROUS_CAPACITY)
        .await
        .expect("append concurrent logs");

    let second_page = store
        .list_http_access_logs(0, true, first_page.next_cursor.clone(), 2, 2)
        .await
        .expect("second page");
    assert_eq!(second_page.total, 6);
    assert_eq!(
        second_page
            .items
            .iter()
            .map(|item| item.request_id)
            .collect::<Vec<_>>(),
        [late_id, middle_id]
    );
    sqlx::query("DELETE FROM http_access_logs WHERE request_id = ?")
        .bind(oldest_id.to_string())
        .execute(store.pool())
        .await
        .expect("delete oldest row between pages");
    let third_page = store
        .list_http_access_logs(0, true, second_page.next_cursor, 3, 2)
        .await
        .expect("third page");
    assert_eq!(third_page.items[0].request_id, older_id);
    assert_eq!(third_page.total, 5);
    assert!(third_page.next_cursor.is_none());
    assert!(
        [first_page.items, second_page.items, third_page.items]
            .into_iter()
            .flatten()
            .all(|item| item.request_id != head_id)
    );
}

#[tokio::test]
async fn incremental_vacuum_reclaims_pages_after_clear_on_new_databases() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-vacuum.sqlite3"))
        .await
        .expect("SQLite store");
    let large = capacity::sized_record(100, 512 * 1024);
    store
        .append_http_access_logs(vec![large], GENEROUS_CAPACITY)
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
        gateway_auth_rejected: false,
        exchange: Some(HttpAccessLogExchange {
            request_headers: vec![HttpHeader {
                name: "x-raw".to_owned(),
                value: vec![0xff, 0x00, b'a'],
            }],
            request_body: HttpBodyCapture::from_vec(b"request body".to_vec(), 12, true, false),
            response_headers: vec![HttpHeader {
                name: "set-cookie".to_owned(),
                value: b"session=raw".to_vec(),
            }],
            response_body: HttpBodyCapture::from_vec(
                b"response body prefix".to_vec(),
                // Deliberately differs from the summary response_bytes so the
                // detail round trip proves the dedicated column is used.
                77,
                true,
                true,
            ),
        }),
    }
}

use any2api_domain::{HttpAccessLog, HttpBodyCapture, RequestId};
use tempfile::tempdir;

use super::record;
use crate::{
    http_access_log::{HttpAccessLogCapacity, HttpAccessLogRepository},
    sqlite::SqliteStore,
};

#[tokio::test]
async fn append_enforces_independent_row_and_exchange_capacity() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-capacity.sqlite3"))
        .await
        .expect("SQLite store");
    let first = sized_record(100, 6);
    let second = sized_record(200, 6);
    let third = sized_record(300, 6);
    let second_id = second.request_id;
    let third_id = third.request_id;

    assert_eq!(
        store
            .append_http_access_logs(
                vec![first, second, third],
                HttpAccessLogCapacity::new(2, 100),
            )
            .await
            .expect("row capacity"),
        1
    );
    assert_eq!(stored_ids(&store).await, vec![second_id, third_id]);

    let fourth = sized_record(400, 16);
    let fourth_id = fourth.request_id;
    assert_eq!(
        store
            .append_http_access_logs(vec![fourth], HttpAccessLogCapacity::new(10, 25),)
            .await
            .expect("exchange capacity"),
        2
    );
    assert_eq!(stored_ids(&store).await, vec![fourth_id]);

    let oversized = sized_record(500, 26);
    assert_eq!(
        store
            .append_http_access_logs(vec![oversized], HttpAccessLogCapacity::new(10, 25),)
            .await
            .expect("oversized exchange"),
        2
    );
    assert!(stored_ids(&store).await.is_empty());
    assert_capacity_stats_match_table(&store).await;
}

#[tokio::test]
async fn gateway_auth_rejected_row_flood_cannot_evict_normal_history() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("auth-rejected-rows.sqlite3"))
        .await
        .expect("SQLite store");
    let capacity = HttpAccessLogCapacity::new(4, 1024 * 1024);
    let mut upstream_unauthorized = record(100, "/v1/responses");
    upstream_unauthorized.status_code = Some(401);
    let protected = [
        upstream_unauthorized,
        record(200, "/v1/responses"),
        record(300, "/api/admin/settings"),
        record(400, "/api/health"),
    ];
    let protected_ids = protected
        .iter()
        .map(|record| record.request_id)
        .collect::<Vec<_>>();
    store
        .append_http_access_logs(Vec::from(protected), capacity)
        .await
        .expect("protected history");

    let rejected = (0..8)
        .map(|index| {
            let mut value = record(500 + index, "/v1/models");
            value.status_code = Some(401);
            value.gateway_auth_rejected = true;
            value
        })
        .collect::<Vec<_>>();
    assert_eq!(
        store
            .append_http_access_logs(rejected, capacity)
            .await
            .expect("rejected flood"),
        8
    );
    assert_eq!(stored_ids(&store).await, protected_ids);
}

#[tokio::test]
async fn gateway_auth_rejected_exchange_budget_evicts_only_its_own_class() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("auth-rejected-bytes.sqlite3"))
        .await
        .expect("SQLite store");
    let capacity = HttpAccessLogCapacity::new(10, 100);
    let protected = sized_record(100, 60);
    let protected_id = protected.request_id;
    let mut rejected = sized_record(200, 50);
    rejected.gateway_auth_rejected = true;

    store
        .append_http_access_logs(vec![protected], capacity)
        .await
        .expect("protected exchange");
    assert_eq!(
        store
            .append_http_access_logs(vec![rejected], capacity)
            .await
            .expect("rejected exchange"),
        1
    );
    assert_eq!(stored_ids(&store).await, vec![protected_id]);
    assert_capacity_stats_match_table(&store).await;
}

async fn assert_capacity_stats_match_table(store: &SqliteStore) {
    let recomputed = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT COUNT(*), COALESCE(SUM(exchange_bytes), 0), \
                COALESCE(SUM(gateway_auth_rejected), 0), \
                COALESCE(SUM(CASE gateway_auth_rejected \
                    WHEN 1 THEN exchange_bytes ELSE 0 END), 0) \
         FROM http_access_logs",
    )
    .fetch_one(store.pool())
    .await
    .expect("recomputed access log aggregates");
    let maintained = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT http_access_log_rows, http_access_log_exchange_bytes, \
                gateway_auth_rejected_rows, gateway_auth_rejected_exchange_bytes \
         FROM telemetry_capacity_stats WHERE singleton_id = 1",
    )
    .fetch_one(store.pool())
    .await
    .expect("maintained capacity stats");
    assert_eq!(maintained, recomputed);
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

pub(super) fn sized_record(started_at_ms: u64, request_body_bytes: usize) -> HttpAccessLog {
    let mut value = record(started_at_ms, "/v1/responses");
    let exchange = value.exchange.as_mut().expect("captured exchange");
    exchange.request_headers.clear();
    exchange.response_headers.clear();
    exchange.request_body = HttpBodyCapture::from_vec(
        vec![b'r'; request_body_bytes],
        u64::try_from(request_body_bytes).expect("request body length fits u64"),
        true,
        false,
    );
    exchange.response_body = HttpBodyCapture::empty(true);
    value
}

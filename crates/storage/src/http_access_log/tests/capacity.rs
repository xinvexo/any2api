use any2api_domain::{HttpAccessLog, RequestId};
use tempfile::tempdir;

use super::record;
use crate::{
    http_access_log::{HttpAccessLogCapacity, HttpAccessLogRepository},
    sqlite::SqliteStore,
};

#[tokio::test]
async fn append_enforces_row_capacity() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("access-log-capacity.sqlite3"))
        .await
        .expect("SQLite store");
    let first = record(100, "/v1/first");
    let second = record(200, "/v1/second");
    let third = record(300, "/v1/third");
    let second_id = second.request_id;
    let third_id = third.request_id;

    assert_eq!(
        store
            .append_http_access_logs(vec![first, second, third], HttpAccessLogCapacity::new(2),)
            .await
            .expect("row capacity"),
        1
    );
    assert_eq!(stored_ids(&store).await, vec![second_id, third_id]);

    assert_capacity_stats_match_table(&store).await;
}

#[tokio::test]
async fn gateway_auth_rejected_row_flood_cannot_evict_normal_history() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("auth-rejected-rows.sqlite3"))
        .await
        .expect("SQLite store");
    let capacity = HttpAccessLogCapacity::new(4);
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

async fn assert_capacity_stats_match_table(store: &SqliteStore) {
    let recomputed = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*), COALESCE(SUM(gateway_auth_rejected), 0) FROM http_access_logs",
    )
    .fetch_one(store.pool())
    .await
    .expect("recomputed access log aggregates");
    let maintained = sqlx::query_as::<_, (i64, i64)>(
        "SELECT http_access_log_rows, gateway_auth_rejected_rows \
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

pub(super) fn sized_record(started_at_ms: u64, metadata_bytes: usize) -> HttpAccessLog {
    let path = format!("/v1/responses/{}", "r".repeat(metadata_bytes));
    let mut value = record(started_at_ms, &path);
    value.method = String::with_capacity(metadata_bytes);
    value.method.push_str("POST");
    value
}

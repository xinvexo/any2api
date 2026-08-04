use any2api_domain::{HttpAccessLog, RequestId};
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
    store
        .append_http_access_logs(&protected, capacity)
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
            .append_http_access_logs(&rejected, capacity)
            .await
            .expect("rejected flood"),
        8
    );
    assert_eq!(
        stored_ids(&store).await,
        protected
            .iter()
            .map(|record| record.request_id)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn gateway_auth_rejected_exchange_budget_evicts_only_its_own_class() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("auth-rejected-bytes.sqlite3"))
        .await
        .expect("SQLite store");
    let capacity = HttpAccessLogCapacity::new(10, 100);
    let protected = sized_record(100, 60);
    let mut rejected = sized_record(200, 50);
    rejected.gateway_auth_rejected = true;

    store
        .append_http_access_logs(std::slice::from_ref(&protected), capacity)
        .await
        .expect("protected exchange");
    assert_eq!(
        store
            .append_http_access_logs(std::slice::from_ref(&rejected), capacity)
            .await
            .expect("rejected exchange"),
        1
    );
    assert_eq!(stored_ids(&store).await, vec![protected.request_id]);
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
    exchange.request_body.content = vec![b'r'; request_body_bytes];
    exchange.request_body.total_bytes =
        u64::try_from(request_body_bytes).expect("request body length fits u64");
    exchange.response_body.content.clear();
    exchange.response_body.total_bytes = 0;
    value
}

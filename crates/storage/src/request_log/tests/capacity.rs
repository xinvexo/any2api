use any2api_domain::RequestId;
use tempfile::tempdir;

use crate::{request_log::RequestLogRepository, sqlite::SqliteStore};

use super::record;

#[tokio::test]
async fn sustained_write_batches_enforce_the_request_log_row_cap() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("write-cap.sqlite3"))
        .await
        .expect("storage");
    const MAX_ROWS: u64 = 100;
    const BATCH_SIZE: u64 = 64;
    const BATCHES: u64 = 20;

    for batch in 0..BATCHES {
        let records = (0..BATCH_SIZE)
            .map(|offset| record(RequestId::new(), batch * BATCH_SIZE + offset + 1, false))
            .collect::<Vec<_>>();
        let cleanup = store
            .append_request_logs(&records, MAX_ROWS)
            .await
            .expect("append capped request logs");
        assert!(!cleanup.has_more());
        assert!(cleanup.deleted_rows() <= BATCH_SIZE);
        assert!(stored_count(&store).await <= MAX_ROWS);
    }

    assert_eq!(stored_count(&store).await, MAX_ROWS);
    let oldest_retained: i64 = sqlx::query_scalar("SELECT MIN(started_at_ms) FROM request_logs")
        .fetch_one(store.pool())
        .await
        .expect("oldest retained timestamp");
    assert_eq!(
        oldest_retained,
        i64::try_from(BATCHES * BATCH_SIZE - MAX_ROWS + 1).expect("timestamp fits i64")
    );
}

#[tokio::test]
async fn a_lower_row_cap_converges_across_bounded_cleanup_transactions() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("bounded-convergence.sqlite3"))
        .await
        .expect("storage");
    let records = (1..=8)
        .map(|started_at_ms| record(RequestId::new(), started_at_ms, false))
        .collect::<Vec<_>>();
    store
        .append_request_logs(&records, 8)
        .await
        .expect("seed request logs");

    for expected in [(3, true, 5), (3, true, 2), (1, false, 1)] {
        let cleanup = store
            .prune_request_logs(0, 1, 3)
            .await
            .expect("bounded cleanup");
        assert_eq!(cleanup.deleted_rows(), expected.0);
        assert_eq!(cleanup.has_more(), expected.1);
        assert_eq!(stored_count(&store).await, expected.2);
    }

    let retained: i64 = sqlx::query_scalar("SELECT started_at_ms FROM request_logs")
        .fetch_one(store.pool())
        .await
        .expect("retained newest request log");
    assert_eq!(retained, 8);
}

#[tokio::test]
async fn capacity_boundary_and_eviction_use_the_narrow_retention_index() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("capacity-plan.sqlite3"))
        .await
        .expect("storage");

    let boundary_plan = explain(
        &store,
        "EXPLAIN QUERY PLAN \
         SELECT started_at_ms, request_id FROM request_logs \
         INDEXED BY request_logs_started_idx \
         ORDER BY started_at_ms DESC, request_id DESC LIMIT 1 OFFSET 99",
    )
    .await;
    assert!(
        boundary_plan
            .iter()
            .any(|detail| detail.contains("COVERING INDEX request_logs_started_idx")),
        "{boundary_plan:?}"
    );

    let eviction_plan = explain(
        &store,
        "EXPLAIN QUERY PLAN \
         DELETE FROM request_logs WHERE request_id IN (\
             SELECT request_id FROM request_logs INDEXED BY request_logs_started_idx \
             WHERE started_at_ms < 10 OR (started_at_ms = 10 AND request_id < 'boundary') \
             ORDER BY started_at_ms ASC, request_id ASC LIMIT 10000\
         )",
    )
    .await;
    assert!(
        eviction_plan
            .iter()
            .any(|detail| detail.contains("COVERING INDEX request_logs_started_idx")),
        "{eviction_plan:?}"
    );
}

async fn stored_count(store: &SqliteStore) -> u64 {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request_logs")
        .fetch_one(store.pool())
        .await
        .expect("request log count");
    u64::try_from(count).expect("non-negative request log count")
}

async fn explain(store: &SqliteStore, statement: &str) -> Vec<String> {
    sqlx::query_as::<_, (i64, i64, i64, String)>(statement)
        .fetch_all(store.pool())
        .await
        .expect("query plan")
        .into_iter()
        .map(|(_, _, _, detail)| detail)
        .collect()
}

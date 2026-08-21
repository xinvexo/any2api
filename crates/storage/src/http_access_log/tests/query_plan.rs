use sqlx::AssertSqlSafe;
use tempfile::tempdir;

use crate::sqlite::SqliteStore;

use super::super::cursor::{
    HIDE_ADMIN_OPERATIONS_PREDICATE, SYSTEM_LOG_BATCH_COLUMNS, SYSTEM_LOG_RETENTION_PREDICATE,
};

#[tokio::test]
async fn system_log_anchor_and_keyset_page_use_the_summary_filter_index() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("system-log-query-plan.sqlite3"))
        .await
        .expect("storage");

    let anchor = explain(
        &store,
        format!(
            "SELECT started_at_ms, request_id FROM http_access_logs \
             INDEXED BY http_access_logs_summary_filter_idx \
             WHERE started_at_ms >= 0 AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
             AND ({HIDE_ADMIN_OPERATIONS_PREDICATE}) \
             ORDER BY started_at_ms DESC, request_id DESC LIMIT 1"
        ),
    )
    .await;
    assert!(
        anchor.contains("USING COVERING INDEX http_access_logs_summary_filter_idx"),
        "{anchor}"
    );
    assert!(!anchor.contains("USE TEMP B-TREE"), "{anchor}");

    let page = explain(
        &store,
        format!(
            "SELECT {SYSTEM_LOG_BATCH_COLUMNS} FROM http_access_logs \
             INDEXED BY http_access_logs_summary_filter_idx \
             WHERE started_at_ms >= 0 AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
             AND ({HIDE_ADMIN_OPERATIONS_PREDICATE}) \
             AND (started_at_ms, request_id) <= (1000, 'ffffffff-ffff-ffff-ffff-ffffffffffff') \
             AND (started_at_ms, request_id) < (500, 'ffffffff-ffff-ffff-ffff-ffffffffffff') \
             ORDER BY started_at_ms DESC, request_id DESC LIMIT 101"
        ),
    )
    .await;
    assert!(
        page.contains("USING INDEX http_access_logs_summary_filter_idx"),
        "{page}"
    );
    assert!(!page.contains("SCAN http_access_logs"), "{page}");
    assert!(!page.contains("USE TEMP B-TREE"), "{page}");
}

async fn explain(store: &SqliteStore, statement: String) -> String {
    sqlx::query_as::<_, (i64, i64, i64, String)>(AssertSqlSafe(format!(
        "EXPLAIN QUERY PLAN {statement}"
    )))
    .fetch_all(store.pool())
    .await
    .expect("query plan")
    .into_iter()
    .map(|(_, _, _, detail)| detail)
    .collect::<Vec<_>>()
    .join("\n")
}

use sqlx::AssertSqlSafe;
use tempfile::tempdir;

use crate::{
    gateway_api_key::GATEWAY_API_KEY_USAGE_SUMMARY_SQL,
    request_log::usage::UPSTREAM_CREDENTIAL_USAGE_SUMMARY_SQL, sqlite::SqliteStore,
};

#[tokio::test]
async fn current_usage_summaries_use_covering_indexes_without_temporary_grouping() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("usage-query-plan.sqlite3"))
        .await
        .expect("storage");

    let upstream = explain(&store, UPSTREAM_CREDENTIAL_USAGE_SUMMARY_SQL).await;
    assert!(
        upstream.contains("COVERING INDEX request_attempts_credential_idx"),
        "{upstream}"
    );
    assert!(
        upstream.contains("COVERING INDEX request_attempts_oauth_account_idx"),
        "{upstream}"
    );
    assert_covering_without_temporary_grouping(&upstream, "request_attempts");

    let gateway = explain(&store, GATEWAY_API_KEY_USAGE_SUMMARY_SQL).await;
    assert!(
        gateway.contains("COVERING INDEX request_logs_gateway_key_started_idx"),
        "{gateway}"
    );
    assert_covering_without_temporary_grouping(&gateway, "request_logs");
}

fn assert_covering_without_temporary_grouping(plan: &str, table: &str) {
    assert!(!plan.contains("USE TEMP B-TREE"), "{plan}");
    let accesses = plan.lines().filter(|line| line.contains(table));
    for access in accesses {
        assert!(access.contains("COVERING INDEX"), "{plan}");
    }
}

async fn explain(store: &SqliteStore, statement: &'static str) -> String {
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

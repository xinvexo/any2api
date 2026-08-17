use any2api_domain::{
    ErrorClass, GatewayApiKeyId, MAX_REQUEST_LOG_ROWS, PublicModelName, RequestId,
    RequestLogFilter, RequestLogOutcomeFilter,
};
use tempfile::tempdir;

use crate::{request_log::RequestLogRepository, sqlite::SqliteStore};

use super::record;

#[tokio::test]
async fn request_log_batches_apply_the_time_window() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("request-log-pages.sqlite3"))
        .await
        .expect("storage");
    let first = record(RequestId::new(), 100, false);
    let second = record(RequestId::new(), 200, false);
    let third = record(RequestId::new(), 300, false);
    store
        .append_request_logs(&[first, second.clone(), third], MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append logs");

    let first_page = store
        .list_request_logs(150, &Default::default(), None, 1)
        .await
        .expect("first visible row");
    let page = store
        .list_request_logs(150, &Default::default(), first_page.next_cursor, 1)
        .await
        .expect("second visible row");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].request_id, second.request.request_id);
}

#[tokio::test]
async fn request_log_cursor_is_stable_across_inserts_and_tail_deletion() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("request-log-cursor.sqlite3"))
        .await
        .expect("storage");
    let oldest = record(RequestId::new(), 100, false);
    let older = record(RequestId::new(), 200, false);
    let middle = record(RequestId::new(), 300, false);
    let fourth = record(RequestId::new(), 400, false);
    let newest = record(RequestId::new(), 500, false);
    store
        .append_request_logs(
            &[
                oldest.clone(),
                older.clone(),
                middle.clone(),
                fourth.clone(),
                newest.clone(),
            ],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append initial logs");

    let first_page = store
        .list_request_logs(0, &Default::default(), None, 2)
        .await
        .expect("first page");
    assert_eq!(
        first_page
            .items
            .iter()
            .map(|item| item.request_id)
            .collect::<Vec<_>>(),
        [newest.request.request_id, fourth.request.request_id]
    );

    let head = record(RequestId::new(), 600, false);
    let late = record(RequestId::new(), 350, false);
    store
        .append_request_logs(&[head.clone(), late.clone()], MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append concurrent logs");

    let second_page = store
        .list_request_logs(0, &Default::default(), first_page.next_cursor.clone(), 2)
        .await
        .expect("second page");
    assert_eq!(
        second_page
            .items
            .iter()
            .map(|item| item.request_id)
            .collect::<Vec<_>>(),
        [late.request.request_id, middle.request.request_id]
    );
    sqlx::query("DELETE FROM request_logs WHERE request_id = ?")
        .bind(oldest.request.request_id.to_string())
        .execute(store.pool())
        .await
        .expect("delete oldest row between pages");
    let third_page = store
        .list_request_logs(0, &Default::default(), second_page.next_cursor, 2)
        .await
        .expect("third page");
    assert_eq!(
        third_page
            .items
            .iter()
            .map(|item| item.request_id)
            .collect::<Vec<_>>(),
        [older.request.request_id]
    );
    assert!(third_page.next_cursor.is_none());
    assert!(
        [first_page.items, second_page.items, third_page.items]
            .into_iter()
            .flatten()
            .all(|item| item.request_id != head.request.request_id)
    );
}

#[tokio::test]
async fn request_log_filters_apply_to_anchor_and_every_cursor_batch() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("request-log-filters.sqlite3"))
        .await
        .expect("storage");
    let gateway_id = GatewayApiKeyId::new();

    let mut success = record(RequestId::new(), 100, false);
    success.request.public_model = Some("filtered-model".into());

    let mut failed = record(RequestId::new(), 200, false);
    failed.request.public_model = Some("filtered-model".into());
    failed.request.status_code = 503;
    failed.request.error_class = Some(ErrorClass::Upstream);

    let mut cancelled = record(RequestId::new(), 300, false);
    cancelled.request.public_model = Some("filtered-model".into());
    cancelled.request.status_code = 499;
    cancelled.request.error_class = Some(ErrorClass::Cancelled);

    let unrelated = record(RequestId::new(), 400, false);
    store
        .append_request_logs(
            &[
                success.clone(),
                failed.clone(),
                cancelled.clone(),
                unrelated,
            ],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append logs");
    let mut connection = store.pool().acquire().await.expect("connection");
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .expect("disable foreign keys for persisted telemetry fixture");
    for request_id in [
        success.request.request_id,
        failed.request.request_id,
        cancelled.request.request_id,
    ] {
        sqlx::query("UPDATE request_logs SET gateway_api_key_id = ? WHERE request_id = ?")
            .bind(gateway_id.to_string())
            .bind(request_id.to_string())
            .execute(&mut *connection)
            .await
            .expect("inject gateway filter ID");
    }
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore foreign keys");
    drop(connection);

    let failed_filter = RequestLogFilter::new(
        Some(RequestLogOutcomeFilter::Failed),
        Some(PublicModelName::new("filtered-model").expect("model")),
        Some(gateway_id),
    );
    let failed_page = store
        .list_request_logs(0, &failed_filter, None, 10)
        .await
        .expect("failed logs");
    assert_eq!(failed_page.items.len(), 1);
    assert_eq!(failed_page.items[0].request_id, failed.request.request_id);

    let cancelled_filter = RequestLogFilter::new(
        Some(RequestLogOutcomeFilter::Cancelled),
        Some(PublicModelName::new("filtered-model").expect("model")),
        Some(gateway_id),
    );
    let cancelled_page = store
        .list_request_logs(0, &cancelled_filter, None, 10)
        .await
        .expect("cancelled logs");
    assert_eq!(cancelled_page.items.len(), 1);
    assert_eq!(
        cancelled_page.items[0].request_id,
        cancelled.request.request_id
    );
}

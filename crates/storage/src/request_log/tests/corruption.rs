use any2api_domain::{MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS, MAX_REQUEST_LOG_ROWS, RequestId};
use tempfile::tempdir;

use crate::{error::StorageError, request_log::RequestLogRepository, sqlite::SqliteStore};

use super::record;

#[tokio::test]
async fn request_log_rejects_a_malformed_persisted_client_ip() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("corrupt-client-ip.sqlite3"))
        .await
        .expect("storage");
    let request_id = RequestId::new();
    store
        .append_request_logs(&[record(request_id, 1_000, false)], MAX_REQUEST_LOG_ROWS)
        .await
        .expect("append request log");
    sqlx::query("UPDATE request_logs SET client_ip = 'not-an-ip' WHERE request_id = ?")
        .bind(request_id.to_string())
        .execute(store.pool())
        .await
        .expect("inject corrupt address");

    assert!(matches!(
        store.get_request_log(request_id).await,
        Err(StorageError::CorruptTelemetry)
    ));
}

#[tokio::test]
async fn request_log_list_skips_a_corrupt_row_without_hiding_it_from_total() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("corrupt-list-row.sqlite3"))
        .await
        .expect("storage");
    let oldest = record(RequestId::new(), 1_000, false);
    let corrupt = record(RequestId::new(), 2_000, false);
    let newest = record(RequestId::new(), 3_000, false);
    store
        .append_request_logs(
            &[oldest.clone(), corrupt.clone(), newest.clone()],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("append request logs");
    sqlx::query(
        "UPDATE request_logs SET error_message = ' invalid diagnostic' WHERE request_id = ?",
    )
    .bind(corrupt.request.request_id.to_string())
    .execute(store.pool())
    .await
    .expect("inject corrupt diagnostic");

    let page = store
        .list_request_logs(0, &Default::default(), None, 10)
        .await
        .expect("list logs");
    assert_eq!(page.total, 3);
    assert_eq!(
        page.items
            .into_iter()
            .map(|item| item.request_id)
            .collect::<Vec<_>>(),
        [newest.request.request_id, oldest.request.request_id]
    );
    assert!(matches!(
        store.get_request_log(corrupt.request.request_id).await,
        Err(StorageError::CorruptTelemetry)
    ));

    let first = store
        .list_request_logs(0, &Default::default(), None, 1)
        .await
        .expect("first cursor page");
    let corrupt_page = store
        .list_request_logs(0, &Default::default(), first.next_cursor, 1)
        .await
        .expect("corrupt cursor page");
    assert!(corrupt_page.items.is_empty());
    let oldest_page = store
        .list_request_logs(0, &Default::default(), corrupt_page.next_cursor, 1)
        .await
        .expect("page after corrupt row");
    assert_eq!(oldest_page.items[0].request_id, oldest.request.request_id);
}

#[tokio::test]
async fn request_log_rejects_unbounded_or_control_character_diagnostics() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("invalid-diagnostics.sqlite3"))
        .await
        .expect("storage");

    let mut oversized = record(RequestId::new(), 1_000, false);
    oversized.request.error_message = Some("a".repeat(MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS + 1));
    assert!(
        store
            .append_request_logs(&[oversized], MAX_REQUEST_LOG_ROWS)
            .await
            .is_err()
    );

    let mut control = record(RequestId::new(), 2_000, false);
    control.request.error_message = Some("unsafe\nmessage".into());
    assert!(
        store
            .append_request_logs(&[control], MAX_REQUEST_LOG_ROWS)
            .await
            .is_err()
    );
}

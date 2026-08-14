use any2api_domain::{
    CredentialId, ErrorClass, GatewayApiKeyId, MAX_REQUEST_LOG_ROWS, OAuthAccountId,
    ProtocolOperation, PublicModelName, RequestId, RequestLogFilter, RequestLogOutcomeFilter,
};
use tempfile::tempdir;

use crate::{request_log::RequestLogRepository, sqlite::SqliteStore};

use super::record;

#[tokio::test]
async fn request_log_pages_apply_the_time_window_before_counting() {
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
    assert_eq!(page.total, 2);
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
    assert_eq!(second_page.total, 6);
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
    assert_eq!(third_page.total, 5);
    assert!(third_page.next_cursor.is_none());
    assert!(
        [first_page.items, second_page.items, third_page.items]
            .into_iter()
            .flatten()
            .all(|item| item.request_id != head.request.request_id)
    );
}

#[tokio::test]
async fn request_log_filters_share_exact_list_and_count_predicates() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("request-log-filters.sqlite3"))
        .await
        .expect("storage");
    let gateway_id = GatewayApiKeyId::new();
    let credential_id = CredentialId::new();
    let oauth_account_id = OAuthAccountId::new();

    let mut success = record(RequestId::new(), 100, false);
    apply_filter_fields(&mut success, gateway_id, ProtocolOperation::Messages);
    success.request.credential_id = Some(credential_id);

    let mut failed = record(RequestId::new(), 200, false);
    apply_filter_fields(&mut failed, gateway_id, ProtocolOperation::Messages);
    failed.request.credential_id = Some(credential_id);
    failed.request.status_code = 503;
    failed.request.error_class = Some(ErrorClass::Upstream);

    let mut cancelled = record(RequestId::new(), 300, false);
    apply_filter_fields(&mut cancelled, gateway_id, ProtocolOperation::Messages);
    cancelled.request.credential_id = Some(credential_id);
    cancelled.request.status_code = 499;
    cancelled.request.error_class = Some(ErrorClass::Cancelled);

    let mut oauth = record(RequestId::new(), 400, false);
    apply_filter_fields(&mut oauth, gateway_id, ProtocolOperation::Messages);
    oauth.request.credential_id = None;
    oauth.request.oauth_account_id = Some(oauth_account_id);
    oauth.request.status_code = 500;
    oauth.request.error_class = Some(ErrorClass::Upstream);

    let unrelated = record(RequestId::new(), 500, false);
    store
        .append_request_logs(
            &[
                success.clone(),
                failed.clone(),
                cancelled.clone(),
                oauth.clone(),
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
        sqlx::query(
            "UPDATE request_logs SET gateway_api_key_id = ?, credential_id = ? \
             WHERE request_id = ?",
        )
        .bind(gateway_id.to_string())
        .bind(credential_id.to_string())
        .bind(request_id.to_string())
        .execute(&mut *connection)
        .await
        .expect("inject provider filter IDs");
    }
    sqlx::query(
        "UPDATE request_logs SET gateway_api_key_id = ?, oauth_account_id = ? \
         WHERE request_id = ?",
    )
    .bind(gateway_id.to_string())
    .bind(oauth_account_id.to_string())
    .bind(oauth.request.request_id.to_string())
    .execute(&mut *connection)
    .await
    .expect("inject OAuth filter IDs");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("restore foreign keys");
    drop(connection);

    let failed_filter = RequestLogFilter::new(
        Some(RequestLogOutcomeFilter::Failed),
        Some(ProtocolOperation::Messages),
        Some(PublicModelName::new("filtered-model").expect("model")),
        Some(gateway_id),
        Some(credential_id),
        None,
    )
    .expect("filter");
    let failed_page = store
        .list_request_logs(0, &failed_filter, None, 10)
        .await
        .expect("failed logs");
    assert_eq!(failed_page.total, 1);
    assert_eq!(failed_page.items[0].request_id, failed.request.request_id);

    let cancelled_filter = RequestLogFilter::new(
        Some(RequestLogOutcomeFilter::Cancelled),
        Some(ProtocolOperation::Messages),
        Some(PublicModelName::new("filtered-model").expect("model")),
        Some(gateway_id),
        Some(credential_id),
        None,
    )
    .expect("filter");
    let cancelled_page = store
        .list_request_logs(0, &cancelled_filter, None, 10)
        .await
        .expect("cancelled logs");
    assert_eq!(cancelled_page.total, 1);
    assert_eq!(
        cancelled_page.items[0].request_id,
        cancelled.request.request_id
    );

    let oauth_filter = RequestLogFilter::new(
        Some(RequestLogOutcomeFilter::Failed),
        Some(ProtocolOperation::Messages),
        Some(PublicModelName::new("filtered-model").expect("model")),
        Some(gateway_id),
        None,
        Some(oauth_account_id),
    )
    .expect("filter");
    let oauth_page = store
        .list_request_logs(0, &oauth_filter, None, 10)
        .await
        .expect("OAuth logs");
    assert_eq!(oauth_page.total, 1);
    assert_eq!(oauth_page.items[0].request_id, oauth.request.request_id);
}

fn apply_filter_fields(
    record: &mut any2api_domain::CompletedRequestLog,
    gateway_id: GatewayApiKeyId,
    operation: ProtocolOperation,
) {
    record.request.gateway_api_key_id = Some(gateway_id);
    record.request.operation = operation;
    record.request.public_model = Some("filtered-model".into());
}

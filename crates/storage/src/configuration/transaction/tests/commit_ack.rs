use std::{sync::mpsc, time::Duration};

use sqlx::{
    Connection, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};
use tempfile::tempdir;

use super::super::{
    SQLITE_PRIMARY_CODE_MASK, commit_error_is_indeterminate, sqlite_primary_code_is_indeterminate,
};

#[test]
fn commit_error_classification_is_conservative_at_the_ack_and_io_boundaries() {
    assert!(commit_error_is_indeterminate(&sqlx::Error::WorkerCrashed));
    assert!(sqlite_primary_code_is_indeterminate(10));
    assert!(sqlite_primary_code_is_indeterminate(
        (10 | (4 << 8)) & SQLITE_PRIMARY_CODE_MASK
    ));
    assert!(!sqlite_primary_code_is_indeterminate(5));
    assert!(!sqlite_primary_code_is_indeterminate(19));
}

#[tokio::test]
async fn commit_hook_rejection_returns_an_error_without_committing() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("commit-rejected.sqlite3");
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .expect("SQLite connection");
    sqlx::query("CREATE TABLE committed_value (value INTEGER NOT NULL)")
        .execute(&mut connection)
        .await
        .expect("test schema");
    connection
        .lock_handle()
        .await
        .expect("locked SQLite handle")
        .set_commit_hook(|| false);

    let mut transaction = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .expect("write transaction");
    sqlx::query("INSERT INTO committed_value (value) VALUES (42)")
        .execute(&mut *transaction)
        .await
        .expect("pending write");
    let error = transaction
        .commit()
        .await
        .expect_err("commit hook must reject COMMIT");

    assert!(matches!(error, sqlx::Error::Database(_)));
    assert!(!commit_error_is_indeterminate(&error));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM committed_value")
        .fetch_one(&mut connection)
        .await
        .expect("rolled-back row count");
    assert_eq!(count, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_commit_waiter_can_lose_acknowledgement_after_sqlite_commits() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("commit-ack.sqlite3");
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .expect("SQLite connection");
    sqlx::query("CREATE TABLE committed_value (value INTEGER NOT NULL)")
        .execute(&mut connection)
        .await
        .expect("test schema");

    let (entered_sender, entered_receiver) = mpsc::sync_channel(1);
    let (release_sender, release_receiver) = mpsc::sync_channel(1);
    connection
        .lock_handle()
        .await
        .expect("locked SQLite handle")
        .set_commit_hook(move || {
            entered_sender.send(()).expect("report commit hook");
            release_receiver.recv().expect("release commit hook");
            true
        });

    let commit_waiter = tokio::spawn(async move {
        let mut transaction = connection
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("write transaction");
        sqlx::query("INSERT INTO committed_value (value) VALUES (42)")
            .execute(&mut *transaction)
            .await
            .expect("pending write");
        transaction.commit().await
    });
    entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("COMMIT reached the SQLite hook");

    commit_waiter.abort();
    let cancellation = commit_waiter
        .await
        .expect_err("the COMMIT waiter must be cancelled");
    assert!(cancellation.is_cancelled());
    release_sender.send(()).expect("allow SQLite COMMIT");

    let committed = tokio::time::timeout(Duration::from_secs(1), async {
        let mut observer = SqliteConnection::connect_with(&options)
            .await
            .expect("observer connection");
        loop {
            match sqlx::query_scalar::<_, i64>("SELECT value FROM committed_value")
                .fetch_optional(&mut observer)
                .await
            {
                Ok(Some(value)) => break value,
                Ok(None) | Err(sqlx::Error::Database(_)) => tokio::task::yield_now().await,
                Err(error) => panic!("observe durable commit: {error}"),
            }
        }
    })
    .await
    .expect("SQLite worker must finish the unacknowledged COMMIT");

    assert_eq!(committed, 42);
}

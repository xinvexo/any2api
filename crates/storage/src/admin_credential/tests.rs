use tempfile::tempdir;

use crate::{
    admin_credential::{AdminCredentialRepository, repository::map_admin_credential_commit_error},
    error::StorageError,
    sqlite::SqliteStore,
    sqlite_commit::SqliteCommitError,
};

#[tokio::test]
async fn administrator_credential_initializes_once_and_survives_reopen() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("any2api.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("sqlite store");

    assert!(store.load_admin_credential().await.expect("load").is_none());
    assert!(
        store
            .initialize_admin_credential("$argon2id$first")
            .await
            .expect("initialize")
    );
    assert!(
        !store
            .initialize_admin_credential("$argon2id$second")
            .await
            .expect("duplicate initialize")
    );
    drop(store);

    let reopened = SqliteStore::connect(&database).await.expect("reopen store");
    let credential = reopened
        .load_admin_credential()
        .await
        .expect("reload")
        .expect("stored credential");
    assert_eq!(credential.password_hash(), "$argon2id$first");

    assert!(
        reopened
            .replace_admin_credential("$argon2id$first", "$argon2id$rotated")
            .await
            .expect("replace")
    );
    assert!(
        !reopened
            .replace_admin_credential("$argon2id$first", "$argon2id$stale")
            .await
            .expect("stale replace")
    );
    assert_eq!(
        reopened
            .load_admin_credential()
            .await
            .expect("load rotated")
            .expect("rotated credential")
            .password_hash(),
        "$argon2id$rotated"
    );
}

#[test]
fn missing_admin_commit_acknowledgement_is_indeterminate() {
    let error = map_admin_credential_commit_error(SqliteCommitError::Indeterminate(
        sqlx::Error::WorkerCrashed,
    ));
    assert!(matches!(
        error,
        StorageError::IndeterminateAdminCredentialCommit { .. }
    ));
}

#[tokio::test]
async fn rejected_admin_initialization_commit_keeps_credential_absent() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("initialize-rejected.sqlite3"))
        .await
        .expect("sqlite store");
    reject_next_commit(&store).await;

    let error = store
        .initialize_admin_credential("$argon2id$first")
        .await
        .expect_err("commit hook must reject initialization");
    assert!(matches!(error, StorageError::Database(_)));
    assert!(store.load_admin_credential().await.expect("load").is_none());
}

#[tokio::test]
async fn rejected_admin_commit_keeps_the_previous_credential() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("commit-rejected.sqlite3"))
        .await
        .expect("sqlite store");
    assert!(
        store
            .initialize_admin_credential("$argon2id$first")
            .await
            .expect("initialize")
    );

    reject_next_commit(&store).await;

    let error = store
        .replace_admin_credential("$argon2id$first", "$argon2id$rotated")
        .await
        .expect_err("commit hook must reject replacement");
    assert!(matches!(error, StorageError::Database(_)));
    let credential = store
        .load_admin_credential()
        .await
        .expect("load")
        .expect("stored credential");
    assert_eq!(credential.password_hash(), "$argon2id$first");
}

async fn reject_next_commit(store: &SqliteStore) {
    let mut connection = store
        .write_pool()
        .acquire()
        .await
        .expect("write connection");
    connection
        .lock_handle()
        .await
        .expect("locked SQLite handle")
        .set_commit_hook(|| false);
}

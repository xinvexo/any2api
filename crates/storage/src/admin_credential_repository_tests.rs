use tempfile::tempdir;

use crate::{admin_credential_repository::AdminCredentialRepository, sqlite::SqliteStore};

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
}

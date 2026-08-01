use std::sync::Arc;

use any2api_domain::{ConfigRevision, OAuthAccountId, ProviderKind, ProxyProfileId};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument, StorageError};

use super::{TestContext, oauth_account_draft, proxy_draft};
use crate::configuration::ConfigPublishError;

#[tokio::test]
async fn commit_reconcile_and_snapshot_switch_share_one_revision() {
    let context = TestContext::new().await;
    let id = ProxyProfileId::new();

    let published = context
        .publisher
        .create_proxy(ConfigRevision::INITIAL, id, proxy_draft("Hong Kong"))
        .await
        .expect("publish proxy");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision().get(), 2);
    assert_eq!(published.revision(), stored.revision());
    assert_eq!(context.snapshots.load().revision(), stored.revision());
    assert!(published.proxies().get(id).is_some());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn stale_publish_is_rejected_before_storage_changes() {
    let context = TestContext::new().await;
    let first_id = ProxyProfileId::new();
    let current = context
        .publisher
        .create_proxy(ConfigRevision::INITIAL, first_id, proxy_draft("Hong Kong"))
        .await
        .expect("first publish");
    let second_id = ProxyProfileId::new();

    let error = context
        .publisher
        .create_proxy(
            ConfigRevision::INITIAL,
            second_id,
            proxy_draft("United States"),
        )
        .await
        .expect_err("stale publish must fail");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert!(matches!(error, ConfigPublishError::RevisionConflict { .. }));
    assert_eq!(stored.revision(), current.revision());
    assert!(stored.proxies().get(second_id).is_none());
    assert_eq!(context.snapshots.load().revision(), current.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn no_op_publish_keeps_revision_and_scheduler_epoch() {
    let context = TestContext::new().await;
    let initial = context.snapshots.load();
    let mut revisions = context.publisher.subscribe_revision();
    assert_eq!(*revisions.borrow_and_update(), ConfigRevision::INITIAL);

    let published = context
        .publisher
        .set_global_proxy(ConfigRevision::INITIAL, ProxyProfileId::DIRECT)
        .await
        .expect("no-op publish");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision(), ConfigRevision::INITIAL);
    assert_eq!(stored.revision(), ConfigRevision::INITIAL);
    assert!(Arc::ptr_eq(&published, &initial));
    assert!(Arc::ptr_eq(&context.snapshots.load(), &initial));
    assert!(
        !revisions
            .has_changed()
            .expect("revision watch remains open")
    );
    assert_eq!(context.runtime.scheduler_epoch(), 0);
}

#[tokio::test]
async fn invalid_oauth_candidate_rolls_back_before_commit_and_snapshot_switch() {
    const ACCESS_TOKEN: &str = "publisher-atomic-secret";

    let context = TestContext::new().await;
    let account_id = OAuthAccountId::new();
    let initial = context.snapshots.load();
    let initial_epoch = context.runtime.scheduler_epoch();
    let mut revisions = context.publisher.subscribe_revision();
    assert_eq!(*revisions.borrow_and_update(), initial.revision());

    let document = OAuthAccountDocument::new(
        ProviderKind::Codex,
        serde_json::to_vec(&serde_json::json!({
            "type": "codex",
            "access_token": ACCESS_TOKEN,
            "expires_in": "not-an-integer",
        }))
        .expect("invalid Provider document JSON")
        .into(),
    )
    .expect("storage accepts the basic OAuth document shape");
    let error = context
        .publisher
        .activate_oauth_account(
            account_id,
            ProviderKind::Codex,
            oauth_account_draft("Rejected OAuth"),
            None,
            None,
            Vec::new(),
            document,
        )
        .await
        .expect_err("Provider compilation must reject the candidate");
    let display = error.to_string();
    let debug = format!("{error:?}");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");
    let current = context.snapshots.load();

    assert!(matches!(
        &error,
        ConfigPublishError::InvalidPublishedSnapshot(_)
    ));
    assert!(display.contains("OAuth document is invalid"));
    assert!(!display.contains(ACCESS_TOKEN));
    assert!(!debug.contains(ACCESS_TOKEN));
    assert_eq!(stored.revision(), initial.revision());
    assert!(stored.oauth_accounts().get(account_id).is_none());
    assert!(Arc::ptr_eq(&current, &initial));
    assert!(
        !revisions
            .has_changed()
            .expect("revision watch remains open")
    );
    assert_eq!(context.runtime.scheduler_epoch(), initial_epoch);
}

#[tokio::test]
async fn commit_failure_keeps_database_snapshot_watch_and_epoch_unchanged() {
    let context = TestContext::new().await;
    let database_path = context.directory.path().join("config.sqlite3");
    install_deferred_commit_failure(&database_path).await;

    let proxy_id = ProxyProfileId::new();
    let initial = context.snapshots.load();
    let initial_epoch = context.runtime.scheduler_epoch();
    let mut revisions = context.publisher.subscribe_revision();
    assert_eq!(*revisions.borrow_and_update(), initial.revision());

    let error = context
        .publisher
        .create_proxy(
            ConfigRevision::INITIAL,
            proxy_id,
            proxy_draft("Commit Failure"),
        )
        .await
        .expect_err("deferred foreign key must reject commit");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");
    let current = context.snapshots.load();

    assert!(matches!(
        &error,
        ConfigPublishError::Internal(StorageError::Database(_))
    ));
    assert_eq!(stored.revision(), initial.revision());
    assert!(stored.proxies().get(proxy_id).is_none());
    assert!(Arc::ptr_eq(&current, &initial));
    assert!(
        !revisions
            .has_changed()
            .expect("revision watch remains open")
    );
    assert_eq!(context.runtime.scheduler_epoch(), initial_epoch);
}

async fn install_deferred_commit_failure(database_path: &std::path::Path) {
    use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .foreign_keys(true);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .expect("commit failure connection");
    for statement in [
        "CREATE TABLE commit_failure_parent (id INTEGER PRIMARY KEY)",
        concat!(
            "CREATE TABLE commit_failure_child (",
            "revision INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL, ",
            "FOREIGN KEY (parent_id) REFERENCES commit_failure_parent(id) ",
            "DEFERRABLE INITIALLY DEFERRED)"
        ),
        concat!(
            "CREATE TRIGGER config_commit_failure AFTER UPDATE OF revision ON config_state ",
            "BEGIN INSERT INTO commit_failure_child (revision, parent_id) ",
            "VALUES (NEW.revision, -1); END"
        ),
    ] {
        sqlx::query(statement)
            .execute(&mut connection)
            .await
            .expect("install commit failure fixture");
    }
    connection
        .close()
        .await
        .expect("close commit failure connection");
}

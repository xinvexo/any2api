use std::future::pending;

use any2api_domain::{ConfigRevision, ProxyAddress, ProxyDraft, ProxyKind, ProxyProfileId};
use tempfile::tempdir;
use tokio::sync::oneshot;

use crate::{
    api::{ConfigurationRepository, SqliteStore},
    error::StorageError,
};

#[tokio::test]
async fn new_database_contains_direct_as_global_proxy() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");

    let configuration = store.load_configuration().await.expect("configuration");

    assert_eq!(configuration.revision(), ConfigRevision::INITIAL);
    assert_eq!(configuration.proxies().profiles().len(), 1);
    assert_eq!(
        configuration.proxies().global_proxy_id(),
        ProxyProfileId::DIRECT
    );
}

#[tokio::test]
async fn proxy_mutations_increment_revision_and_protect_global_proxy() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let id = ProxyProfileId::new();
    let address = ProxyAddress::new("proxy.example.com", 1080).expect("address");
    let draft = ProxyDraft::new("Hong Kong", ProxyKind::Socks5, address, true).expect("draft");

    let created = store
        .create_proxy(ConfigRevision::INITIAL, id, draft)
        .await
        .expect("create proxy");
    let global = store
        .set_global_proxy(created.revision(), id)
        .await
        .expect("set global");
    let error = store
        .delete_proxy(global.revision(), id)
        .await
        .expect_err("global proxy cannot be deleted");

    assert_eq!(global.revision().get(), 3);
    assert!(matches!(error, StorageError::ProxyInUse));
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("configuration")
            .revision(),
        global.revision()
    );
}

#[tokio::test]
async fn current_global_proxy_cannot_be_disabled() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let id = ProxyProfileId::new();
    let enabled = proxy_draft("Hong Kong", true);
    let created = store
        .create_proxy(ConfigRevision::INITIAL, id, enabled)
        .await
        .expect("create proxy");
    let global = store
        .set_global_proxy(created.revision(), id)
        .await
        .expect("set global");

    let error = store
        .update_proxy(global.revision(), id, proxy_draft("Hong Kong", false))
        .await
        .expect_err("global proxy cannot be disabled");
    let stored = store.load_configuration().await.expect("configuration");

    assert!(matches!(error, StorageError::ProxyInUse));
    assert_eq!(stored.revision(), global.revision());
    assert!(stored.proxies().get(id).expect("proxy").enabled());
}

#[tokio::test]
async fn proxy_authentication_is_encrypted_versioned_and_clearable() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let id = ProxyProfileId::new();
    let created = store
        .create_proxy(
            ConfigRevision::INITIAL,
            id,
            proxy_draft("Authenticated", true),
        )
        .await
        .expect("create proxy");

    let authenticated = store
        .set_proxy_authentication(
            created.revision(),
            id,
            "proxy-user".to_owned(),
            b"proxy-password".to_vec().into(),
        )
        .await
        .expect("set proxy authentication");
    let profile = authenticated.proxies().get(id).expect("profile");
    assert_eq!(
        profile.authentication().expect("authentication").username(),
        "proxy-user"
    );
    assert_eq!(profile.authentication_version(), 1);
    assert_eq!(
        authenticated
            .proxy_passwords()
            .get(id)
            .expect("stored password")
            .expose_for_test(),
        b"proxy-password"
    );
    assert!(!format!("{authenticated:?}").contains("proxy-password"));

    let replaced = store
        .set_proxy_authentication(
            authenticated.revision(),
            id,
            "proxy-user-2".to_owned(),
            b"replacement".to_vec().into(),
        )
        .await
        .expect("replace proxy authentication");
    assert_eq!(
        replaced
            .proxies()
            .get(id)
            .expect("profile")
            .authentication_version(),
        2
    );

    drop(store);
    let store = SqliteStore::connect(&database).await.expect("reopen store");
    let reloaded_authenticated = store
        .load_configuration()
        .await
        .expect("reload authenticated configuration");
    assert_eq!(
        reloaded_authenticated
            .proxies()
            .get(id)
            .expect("reloaded profile")
            .authentication_version(),
        2
    );
    assert_eq!(
        reloaded_authenticated
            .proxy_passwords()
            .get(id)
            .expect("reloaded password")
            .expose_for_test(),
        b"replacement"
    );

    let cleared = store
        .clear_proxy_authentication(reloaded_authenticated.revision(), id)
        .await
        .expect("clear proxy authentication");
    let profile = cleared.proxies().get(id).expect("profile");
    assert!(profile.authentication().is_none());
    assert_eq!(profile.authentication_version(), 3);
    assert!(cleared.proxy_passwords().get(id).is_none());

    let repeated = store
        .clear_proxy_authentication(cleared.revision(), id)
        .await
        .expect("repeat clear proxy authentication");
    assert_eq!(repeated.revision(), cleared.revision());
    assert_eq!(
        repeated
            .proxies()
            .get(id)
            .expect("profile")
            .authentication_version(),
        3
    );

    drop(store);
    let reloaded = SqliteStore::connect(&database).await.expect("reopen store");
    assert!(
        reloaded
            .load_configuration()
            .await
            .expect("reloaded configuration")
            .proxies()
            .get(id)
            .expect("reloaded profile")
            .authentication()
            .is_none()
    );
}

#[tokio::test]
async fn dropping_an_immediate_transaction_releases_the_sqlite_writer() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let pool = store.pool().clone();
    let (ready_tx, ready_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let _transaction = pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .expect("immediate transaction");
        ready_tx.send(()).expect("signal transaction");
        pending::<()>().await;
    });
    ready_rx.await.expect("transaction started");
    task.abort();
    let _ = task.await;

    let created = store
        .create_proxy(
            ConfigRevision::INITIAL,
            ProxyProfileId::new(),
            proxy_draft("After Cancellation", true),
        )
        .await
        .expect("writer must be released");

    assert_eq!(created.revision().get(), 2);
}

#[tokio::test]
async fn proxy_settings_singleton_cannot_be_deleted() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");

    let error = sqlx::query("DELETE FROM proxy_settings WHERE singleton_id = 1")
        .execute(store.pool())
        .await
        .expect_err("proxy settings singleton must be protected");
    let stored = store.load_configuration().await.expect("configuration");

    assert!(error.to_string().contains("proxy_settings_immutable"));
    assert_eq!(stored.proxies().global_proxy_id(), ProxyProfileId::DIRECT);
}

fn proxy_draft(name: &str, enabled: bool) -> ProxyDraft {
    let address = ProxyAddress::new("proxy.example.com", 1080).expect("address");
    ProxyDraft::new(name, ProxyKind::Socks5, address, enabled).expect("draft")
}

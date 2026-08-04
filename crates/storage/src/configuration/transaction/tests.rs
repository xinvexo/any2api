use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use any2api_domain::{
    ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId, ProxyAddress, ProxyDraft, ProxyKind,
    ProxyProfileId, RateLimitMode, SettingKey, SettingOverrideChange, SettingValue,
};
use tempfile::tempdir;

use crate::{
    configuration::{
        ConfigurationMutation, ConfigurationRepository, ConfigurationTransactionOutcome,
        ConfigurationTransactionRepository, StoredConfiguration, commit_configuration,
    },
    error::StorageError,
    secret::SecretBytes,
    sqlite::SqliteStore,
};

mod commit_ack;

#[tokio::test]
async fn accepted_candidate_is_committed_before_it_is_returned() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");

    let outcome = accept_candidate(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerOnRateLimited,
                value: SettingValue::RateLimitMode(RateLimitMode::Reject),
            }],
        },
    )
    .await
    .expect("accepted transaction");
    let ConfigurationTransactionOutcome::Committed(candidate) = outcome else {
        panic!("changed candidate must commit");
    };

    assert_eq!(candidate.revision().get(), 2);
    assert_eq!(
        candidate.settings().scheduler().on_rate_limited(),
        RateLimitMode::Reject
    );
    let committed = store
        .load_configuration()
        .await
        .expect("committed configuration");
    assert_eq!(committed.revision(), candidate.revision());
    assert_eq!(
        committed.settings().scheduler().on_rate_limited(),
        RateLimitMode::Reject
    );
}

#[tokio::test]
async fn rejected_candidate_is_rolled_back_before_it_is_returned() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");

    let outcome = <SqliteStore as ConfigurationTransactionRepository<
        Infallible,
        ConfigRevision,
    >>::transact_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerFallbackOnRateLimit,
                value: SettingValue::Boolean(true),
            }],
        },
        Box::new(|candidate| {
            assert_eq!(candidate.revision().get(), 2);
            assert_eq!(
                candidate
                    .settings()
                    .override_value(SettingKey::SchedulerFallbackOnRateLimit),
                Some(SettingValue::Boolean(true))
            );
            Err(candidate.revision())
        }),
    )
    .await
    .expect("rejected transaction");

    assert!(matches!(
        outcome,
        ConfigurationTransactionOutcome::Rejected(revision) if revision.get() == 2
    ));
    let current = store
        .load_configuration()
        .await
        .expect("current configuration");
    assert_eq!(current.revision(), ConfigRevision::INITIAL);
    assert_eq!(
        current
            .settings()
            .override_value(SettingKey::SchedulerFallbackOnRateLimit),
        None
    );
}

#[tokio::test]
async fn no_op_rolls_back_without_invoking_the_candidate_compiler() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");
    let invoked = Arc::new(AtomicBool::new(false));
    let invoked_by_compiler = Arc::clone(&invoked);

    let outcome = <SqliteStore as ConfigurationTransactionRepository<(), Infallible>>::
        transact_configuration(
            &store,
            ConfigRevision::INITIAL,
            ConfigurationMutation::ApplySettingChanges {
                changes: vec![SettingOverrideChange::Reset {
                    key: SettingKey::SchedulerFallbackOnRateLimit,
                }],
            },
            Box::new(move |_| {
                invoked_by_compiler.store(true, Ordering::SeqCst);
                Ok(())
            }),
        )
        .await
        .expect("no-op transaction");

    assert!(matches!(outcome, ConfigurationTransactionOutcome::NoChange));
    assert!(!invoked.load(Ordering::SeqCst));
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("current configuration")
            .revision(),
        ConfigRevision::INITIAL
    );
}

#[tokio::test]
async fn commit_failure_drops_the_compiled_value_and_returns_storage_error() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("configuration.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    install_deferred_commit_failure(&database).await;
    let dropped = Arc::new(AtomicBool::new(false));
    let dropped_by_value = Arc::clone(&dropped);

    let result = <SqliteStore as ConfigurationTransactionRepository<DropProbe, Infallible>>::
        transact_configuration(
            &store,
            ConfigRevision::INITIAL,
            ConfigurationMutation::CreateProxy {
                id: ProxyProfileId::new(),
                draft: proxy_draft("Commit failure"),
            },
            Box::new(move |_| Ok(DropProbe(dropped_by_value))),
        )
        .await;
    let Err(error) = result else {
        panic!("deferred foreign key must reject commit");
    };

    assert!(matches!(error, StorageError::Database(_)));
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("rolled-back configuration")
            .revision(),
        ConfigRevision::INITIAL
    );
}

#[tokio::test]
async fn rejected_proxy_candidate_still_contains_complete_gateway_configuration() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");
    let key_id = GatewayApiKeyId::new();
    let token = format!(
        "{}{}",
        any2api_domain::GATEWAY_TOKEN_PREFIX,
        "p".repeat(any2api_domain::GATEWAY_TOKEN_BODY_LEN)
    );
    let with_key = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateGatewayApiKey {
            id: key_id,
            draft: GatewayApiKeyDraft::new("Existing key", true).expect("key draft"),
            token: SecretBytes::from(token.clone().into_bytes()),
        },
    )
    .await
    .expect("create gateway key");
    let proxy_id = ProxyProfileId::new();
    let with_proxy = commit_configuration(
        &store,
        with_key.revision(),
        ConfigurationMutation::CreateProxy {
            id: proxy_id,
            draft: proxy_draft("Before"),
        },
    )
    .await
    .expect("create proxy");

    let outcome = <SqliteStore as ConfigurationTransactionRepository<
        Infallible,
        StoredConfiguration,
    >>::transact_configuration(
        &store,
        with_proxy.revision(),
        ConfigurationMutation::UpdateProxy {
            id: proxy_id,
            draft: proxy_draft("After"),
        },
        Box::new(Err),
    )
    .await
    .expect("reject proxy candidate");
    let ConfigurationTransactionOutcome::Rejected(candidate) = outcome else {
        panic!("changed proxy candidate must reach the compiler");
    };

    assert_eq!(
        candidate
            .proxies()
            .get(proxy_id)
            .expect("updated proxy")
            .name(),
        "After"
    );
    let key = candidate
        .gateway_api_keys()
        .get(key_id)
        .expect("unchanged gateway key");
    assert_eq!(key.token(), token);
    assert!(
        candidate
            .gateway_api_key_verifier()
            .verify(token.as_bytes(), key.token_hash())
    );
    let committed = store
        .load_configuration()
        .await
        .expect("rolled-back configuration");
    assert_eq!(committed.revision(), with_proxy.revision());
    assert_eq!(
        committed
            .proxies()
            .get(proxy_id)
            .expect("committed proxy")
            .name(),
        "Before"
    );
}

async fn accept_candidate(
    store: &SqliteStore,
    expected: ConfigRevision,
    mutation: ConfigurationMutation,
) -> Result<ConfigurationTransactionOutcome<StoredConfiguration, Infallible>, StorageError> {
    <SqliteStore as ConfigurationTransactionRepository<StoredConfiguration, Infallible>>::
        transact_configuration(
            store,
            expected,
            mutation,
            Box::new(Ok),
        )
        .await
}

async fn install_deferred_commit_failure(database: &std::path::Path) {
    use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

    let options = SqliteConnectOptions::new()
        .filename(database)
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

struct DropProbe(Arc<AtomicBool>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn proxy_draft(name: &str) -> ProxyDraft {
    ProxyDraft::new(
        name,
        ProxyKind::Http,
        ProxyAddress::new("proxy.example.com", 8080).expect("proxy address"),
        true,
    )
    .expect("proxy draft")
}

use std::{panic::AssertUnwindSafe, sync::Arc, time::Duration};

use any2api_domain::{ConfigRevision, ProxyProfileId};
use any2api_storage::api::{
    ConfigurationCandidateCompiler, ConfigurationMutation, ConfigurationRepository,
    ConfigurationTransactionOutcome, ConfigurationTransactionRepository, SqliteStore, StorageError,
    StoredConfiguration,
};
use async_trait::async_trait;
use futures_util::FutureExt;
use tokio::sync::Notify;

use super::{TestContext, proxy_draft};
use crate::configuration::{ConfigPublishError, ConfigPublisher, PreparedPublishedSnapshot};

#[tokio::test]
async fn indeterminate_commit_panics_in_the_critical_publish_stage() {
    let context = TestContext::new().await;
    let repository = Arc::new(IndeterminateCommitRepository(Arc::clone(
        &context.repository,
    )));
    let publisher = ConfigPublisher::new(
        repository,
        Arc::clone(&context.snapshots),
        Arc::clone(&context.runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let initial = context.snapshots.load();
    let proxy_id = ProxyProfileId::new();

    let result = AssertUnwindSafe(publisher.publish_mutation_serialized(
        Arc::clone(&initial),
        initial.revision(),
        ConfigurationMutation::CreateProxy {
            id: proxy_id,
            draft: proxy_draft("Indeterminate Commit"),
        },
    ))
    .catch_unwind()
    .await;

    assert!(result.is_err(), "indeterminate commit must be fatal");
    assert!(Arc::ptr_eq(&context.snapshots.load(), &initial));
    assert_eq!(context.runtime.scheduler_epoch(), 0);
    assert_eq!(
        context
            .repository
            .load_configuration()
            .await
            .expect("unchanged fixture repository")
            .revision(),
        ConfigRevision::INITIAL
    );
}

#[tokio::test]
async fn cancelled_waiter_after_sqlite_commit_still_switches_the_snapshot() {
    let context = TestContext::new().await;
    let repository = Arc::new(PauseAfterCommitRepository::new(Arc::clone(
        &context.repository,
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&repository),
        Arc::clone(&context.snapshots),
        Arc::clone(&context.runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let proxy_id = ProxyProfileId::new();
    let mut revisions = context.publisher.subscribe_revision();

    let publish_waiter = tokio::spawn(async move {
        publisher
            .create_proxy(
                ConfigRevision::INITIAL,
                proxy_id,
                proxy_draft("Detached Commit"),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), repository.committed.notified())
        .await
        .expect("SQLite commit must complete");

    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("committed configuration");
    assert_eq!(stored.revision().get(), 2);
    assert!(stored.proxies().get(proxy_id).is_some());
    assert_eq!(context.snapshots.load().revision(), ConfigRevision::INITIAL);

    publish_waiter.abort();
    assert!(
        publish_waiter
            .await
            .expect_err("the HTTP-style waiter must be cancelled")
            .is_cancelled()
    );
    repository.release.notify_one();

    tokio::time::timeout(Duration::from_secs(1), revisions.changed())
        .await
        .expect("detached publish must switch the snapshot")
        .expect("revision watch remains open");
    let current = context.snapshots.load();
    assert_eq!(current.revision(), stored.revision());
    assert!(current.proxies().get(proxy_id).is_some());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

struct PauseAfterCommitRepository {
    inner: Arc<SqliteStore>,
    committed: Notify,
    release: Notify,
}

struct IndeterminateCommitRepository(Arc<SqliteStore>);

#[async_trait]
impl ConfigurationRepository for IndeterminateCommitRepository {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError> {
        self.0.load_configuration().await
    }
}

#[async_trait]
impl ConfigurationTransactionRepository<PreparedPublishedSnapshot, ConfigPublishError>
    for IndeterminateCommitRepository
{
    async fn transact_configuration(
        &self,
        _expected: ConfigRevision,
        _mutation: ConfigurationMutation,
        _compiler: ConfigurationCandidateCompiler<PreparedPublishedSnapshot, ConfigPublishError>,
    ) -> Result<
        ConfigurationTransactionOutcome<PreparedPublishedSnapshot, ConfigPublishError>,
        StorageError,
    > {
        Err(StorageError::IndeterminateConfigurationCommit {
            source: sqlx::Error::WorkerCrashed,
        })
    }
}

impl PauseAfterCommitRepository {
    fn new(inner: Arc<SqliteStore>) -> Self {
        Self {
            inner,
            committed: Notify::new(),
            release: Notify::new(),
        }
    }
}

#[async_trait]
impl ConfigurationRepository for PauseAfterCommitRepository {
    async fn load_configuration(&self) -> Result<StoredConfiguration, StorageError> {
        self.inner.load_configuration().await
    }
}

#[async_trait]
impl ConfigurationTransactionRepository<PreparedPublishedSnapshot, ConfigPublishError>
    for PauseAfterCommitRepository
{
    async fn transact_configuration(
        &self,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
        compiler: ConfigurationCandidateCompiler<PreparedPublishedSnapshot, ConfigPublishError>,
    ) -> Result<
        ConfigurationTransactionOutcome<PreparedPublishedSnapshot, ConfigPublishError>,
        StorageError,
    > {
        let outcome = <SqliteStore as ConfigurationTransactionRepository<
            PreparedPublishedSnapshot,
            ConfigPublishError,
        >>::transact_configuration(
            self.inner.as_ref(), expected, mutation, compiler
        )
        .await?;
        if matches!(outcome, ConfigurationTransactionOutcome::Committed(_)) {
            self.committed.notify_one();
            self.release.notified().await;
        }
        Ok(outcome)
    }
}

//! Serialized configuration publication coordinator.

use std::sync::Arc;

use any2api_domain::ConfigRevision;
use any2api_storage::api::{
    ConfigurationMutation, ConfigurationTransactionOutcome, ConfigurationTransactionRepository,
    StorageError, StoredConfiguration,
};
use tokio::sync::watch;

use crate::{
    configuration::{
        ConfigPublishError, ConfigurationCapabilities, PreparedPublishedSnapshot,
        PublicationSource, PublishedSnapshot, PublishedSnapshotReconciler, SnapshotStore,
        command::ConfigCommand, publish_task,
    },
    registry::RuntimeRegistry,
};

#[derive(Clone)]
pub struct ConfigPublisher {
    pub(crate) repository:
        Arc<dyn ConfigurationTransactionRepository<PreparedPublishedSnapshot, ConfigPublishError>>,
    pub(crate) snapshots: Arc<SnapshotStore>,
    pub(crate) runtime: Arc<RuntimeRegistry>,
    pub(super) capabilities: Arc<ConfigurationCapabilities>,
    snapshot_reconciler: Option<Arc<dyn PublishedSnapshotReconciler>>,
}

impl ConfigPublisher {
    pub fn new<R>(
        repository: Arc<R>,
        snapshots: Arc<SnapshotStore>,
        runtime: Arc<RuntimeRegistry>,
        capabilities: Arc<ConfigurationCapabilities>,
    ) -> Result<Self, ConfigPublishError>
    where
        R: ConfigurationTransactionRepository<PreparedPublishedSnapshot, ConfigPublishError>
            + 'static,
    {
        let current = snapshots.load();
        capabilities.validate_configuration(
            current.provider_endpoints(),
            current.provider_credentials(),
            current.model_routes(),
        )?;
        runtime.publish_route_admissions(current.as_ref());
        Ok(Self {
            repository,
            snapshots,
            runtime,
            capabilities,
            snapshot_reconciler: None,
        })
    }

    #[must_use]
    pub fn with_snapshot_reconciler(
        mut self,
        reconciler: Arc<dyn PublishedSnapshotReconciler>,
    ) -> Self {
        self.snapshot_reconciler = Some(reconciler);
        self
    }

    #[must_use]
    pub fn configuration_capabilities(&self) -> &ConfigurationCapabilities {
        self.capabilities.as_ref()
    }

    pub(crate) fn current_snapshot(&self) -> Arc<PublishedSnapshot> {
        self.snapshots.load()
    }

    pub(crate) fn subscribe_revision(&self) -> watch::Receiver<ConfigRevision> {
        self.snapshots.subscribe_revision()
    }

    pub(crate) async fn publish(
        &self,
        expected: ConfigRevision,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            publisher.publish_serialized(expected, command).await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn publish_serialized(
        &self,
        expected: ConfigRevision,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        let actual = current.revision();
        let transaction_revision = if actual == expected {
            expected
        } else if self
            .snapshots
            .can_rebase_after_automatic_publications(expected, actual)
        {
            actual
        } else {
            return Err(ConfigPublishError::RevisionConflict { expected, actual });
        };
        self.validate_command(current.as_ref(), &command)?;
        let (published, _) = self
            .publish_mutation_serialized_with_source(
                current,
                transaction_revision,
                command.into_mutation(),
                PublicationSource::Operator,
            )
            .await?;
        Ok(published)
    }

    pub(super) async fn publish_current(
        &self,
        source: PublicationSource,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            publisher.publish_current_serialized(source, command).await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn publish_current_serialized(
        &self,
        source: PublicationSource,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        let expected = current.revision();
        self.validate_command(current.as_ref(), &command)?;
        let (published, _) = self
            .publish_mutation_serialized_with_source(
                current,
                expected,
                command.into_mutation(),
                source,
            )
            .await?;
        Ok(published)
    }

    pub(super) async fn publish_mutation_serialized(
        &self,
        current: Arc<PublishedSnapshot>,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
    ) -> Result<(Arc<PublishedSnapshot>, bool), ConfigPublishError> {
        self.publish_mutation_serialized_with_source(
            current,
            expected,
            mutation,
            PublicationSource::Operator,
        )
        .await
    }

    pub(super) async fn publish_mutation_serialized_with_source(
        &self,
        current: Arc<PublishedSnapshot>,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
        source: PublicationSource,
    ) -> Result<(Arc<PublishedSnapshot>, bool), ConfigPublishError> {
        let capabilities = Arc::clone(&self.capabilities);
        let outcome = match self
            .repository
            .transact_configuration(
                expected,
                mutation,
                Box::new(move |candidate| {
                    Self::compile_candidate(capabilities.as_ref(), expected, candidate)
                }),
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(StorageError::IndeterminateConfigurationCommit { .. }) => {
                panic!("configuration commit outcome is indeterminate; process cannot continue")
            }
            Err(error) => return Err(error.into()),
        };
        let prepared_snapshot = match outcome {
            ConfigurationTransactionOutcome::NoChange => return Ok((current, false)),
            ConfigurationTransactionOutcome::Committed(prepared_snapshot) => prepared_snapshot,
            ConfigurationTransactionOutcome::Rejected(error) => return Err(error),
        };

        let published = self
            .snapshots
            .replace(prepared_snapshot.bind(&self.runtime), source);
        self.runtime.publish_route_admissions(published.as_ref());
        if let Some(reconciler) = &self.snapshot_reconciler {
            reconciler.reconcile(published.as_ref());
        }
        self.runtime.advance_scheduler_epoch();
        Ok((published, true))
    }

    fn compile_candidate(
        capabilities: &ConfigurationCapabilities,
        expected: ConfigRevision,
        candidate: StoredConfiguration,
    ) -> Result<PreparedPublishedSnapshot, ConfigPublishError> {
        let next = expected
            .checked_next()
            .map_err(|_| ConfigPublishError::RevisionOverflow)?;
        if candidate.revision() != next {
            return Err(ConfigPublishError::UnexpectedCandidateRevision {
                expected: next,
                actual: candidate.revision(),
            });
        }
        capabilities.validate_configuration(
            candidate.provider_endpoints(),
            candidate.provider_credentials(),
            candidate.model_routes(),
        )?;
        PreparedPublishedSnapshot::compile(candidate, capabilities.provider_registry())
            .map_err(Into::into)
    }
}

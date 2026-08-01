//! Serialized configuration publication coordinator.

use std::sync::Arc;

use any2api_domain::ConfigRevision;
use any2api_storage::api::{ConfigurationMutation, ConfigurationRepository, StoredConfiguration};
use tokio::sync::watch;

use crate::{
    configuration::{
        ConfigPublishError, ConfigurationCapabilities, LoggingSettingsReconciler,
        PreparedPublishedSnapshot, PublishedSnapshot, SnapshotStore, command::ConfigCommand,
        publish_task,
    },
    registry::RuntimeRegistry,
};

#[derive(Clone)]
pub struct ConfigPublisher {
    pub(crate) repository: Arc<dyn ConfigurationRepository>,
    pub(crate) snapshots: Arc<SnapshotStore>,
    pub(crate) runtime: Arc<RuntimeRegistry>,
    pub(super) capabilities: Arc<ConfigurationCapabilities>,
    logging_reconciler: Option<Arc<dyn LoggingSettingsReconciler>>,
}

impl ConfigPublisher {
    pub fn new<R>(
        repository: Arc<R>,
        snapshots: Arc<SnapshotStore>,
        runtime: Arc<RuntimeRegistry>,
        capabilities: Arc<ConfigurationCapabilities>,
    ) -> Result<Self, ConfigPublishError>
    where
        R: ConfigurationRepository + 'static,
    {
        let current = snapshots.load();
        capabilities.validate_configuration(
            current.provider_endpoints(),
            current.provider_credentials(),
            current.model_routes(),
        )?;
        Ok(Self {
            repository,
            snapshots,
            runtime,
            capabilities,
            logging_reconciler: None,
        })
    }

    #[must_use]
    pub fn with_logging_reconciler(
        mut self,
        reconciler: Arc<dyn LoggingSettingsReconciler>,
    ) -> Self {
        self.logging_reconciler = Some(reconciler);
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
        if current.revision() != expected {
            return Err(ConfigPublishError::RevisionConflict {
                expected,
                actual: current.revision(),
            });
        }
        self.validate_command(current.as_ref(), &command)?;
        let (published, _) = self
            .publish_mutation_serialized(current, expected, command.into_mutation())
            .await?;
        Ok(published)
    }

    pub(super) async fn publish_current(
        &self,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            publisher.publish_current_serialized(command).await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn publish_current_serialized(
        &self,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        let expected = current.revision();
        self.validate_command(current.as_ref(), &command)?;
        let (published, _) = self
            .publish_mutation_serialized(current, expected, command.into_mutation())
            .await?;
        Ok(published)
    }

    pub(super) async fn publish_mutation_serialized(
        &self,
        current: Arc<PublishedSnapshot>,
        expected: ConfigRevision,
        mutation: ConfigurationMutation,
    ) -> Result<(Arc<PublishedSnapshot>, bool), ConfigPublishError> {
        let prepared = self
            .repository
            .prepare_configuration(expected, mutation)
            .await?;
        let changed = prepared.changed();
        let (candidate, commit) = prepared.into_parts();
        if !changed {
            commit.finish().await?;
            return Ok((current, false));
        }

        let prepared_snapshot = match self.compile_candidate(expected, candidate) {
            Ok(prepared_snapshot) => prepared_snapshot,
            Err(error) => {
                commit.rollback().await?;
                return Err(error);
            }
        };
        commit.finish().await?;

        let published = self
            .snapshots
            .replace(prepared_snapshot.bind(&self.runtime));
        if let Some(reconciler) = &self.logging_reconciler {
            reconciler.reconcile(published.revision(), published.settings().logging());
        }
        self.runtime.advance_scheduler_epoch();
        Ok((published, true))
    }

    fn compile_candidate(
        &self,
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
        self.capabilities.validate_configuration(
            candidate.provider_endpoints(),
            candidate.provider_credentials(),
            candidate.model_routes(),
        )?;
        PreparedPublishedSnapshot::compile(candidate, self.capabilities.provider_registry())
            .map_err(Into::into)
    }
}

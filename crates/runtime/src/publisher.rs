use std::sync::Arc;

use any2api_domain::ConfigRevision;
use any2api_storage::api::ConfigurationRepository;

use crate::{
    config_command::{ConfigCommand, execute},
    config_publish_error::ConfigPublishError,
    configuration_capabilities::ConfigurationCapabilities,
    logging_reconciler::LoggingSettingsReconciler,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

mod providers;
mod proxies;
mod settings;

#[derive(Clone)]
pub struct ConfigPublisher {
    pub(crate) repository: Arc<dyn ConfigurationRepository>,
    pub(crate) snapshots: Arc<SnapshotStore>,
    pub(crate) runtime: Arc<RuntimeRegistry>,
    capabilities: Arc<ConfigurationCapabilities>,
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

    async fn publish(
        &self,
        expected: ConfigRevision,
        command: ConfigCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        crate::publish_task::run(self.runtime.lifecycle(), async move {
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
        let committed = execute(self.repository.as_ref(), expected, command).await?;
        Ok(self.publish_committed(current, expected, committed))
    }

    pub(crate) fn publish_committed(
        &self,
        current: Arc<PublishedSnapshot>,
        expected: ConfigRevision,
        committed: any2api_storage::api::StoredConfiguration,
    ) -> Arc<PublishedSnapshot> {
        if committed.revision() == expected {
            return current;
        }
        let next = expected
            .checked_next()
            .expect("repository committed a revision after the persisted maximum");
        assert_eq!(
            committed.revision(),
            next,
            "repository committed an unexpected configuration revision"
        );
        let snapshot = PublishedSnapshot::new(committed, self.runtime.as_ref());
        if let Some(reconciler) = &self.logging_reconciler {
            reconciler.reconcile(snapshot.revision(), snapshot.settings().logging());
        }
        let published = self.snapshots.replace(snapshot);
        self.runtime.advance_scheduler_epoch();
        published
    }
}

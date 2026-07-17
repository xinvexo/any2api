use std::sync::Arc;

use any2api_domain::{ConfigRevision, ProxyDraft, ProxyProfileId, ProxyValidationError};
use any2api_storage::api::{ConfigurationRepository, StorageError, StoredConfiguration};
use thiserror::Error;

use crate::{
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

pub struct ConfigPublisher {
    repository: Arc<dyn ConfigurationRepository>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
}

impl ConfigPublisher {
    #[must_use]
    pub fn new<R>(
        repository: Arc<R>,
        snapshots: Arc<SnapshotStore>,
        runtime: Arc<RuntimeRegistry>,
    ) -> Self
    where
        R: ConfigurationRepository + 'static,
    {
        Self {
            repository,
            snapshots,
            runtime,
        }
    }

    pub async fn create_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ProxyCommand::Create { id, draft })
            .await
    }

    pub async fn update_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ProxyCommand::Update { id, draft })
            .await
    }

    pub async fn delete_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ProxyCommand::Delete { id }).await
    }

    pub async fn set_global_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ProxyCommand::SetGlobal { id }).await
    }

    async fn publish(
        &self,
        expected: ConfigRevision,
        command: ProxyCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        if current.revision() != expected {
            return Err(ConfigPublishError::RevisionConflict {
                expected,
                actual: current.revision(),
            });
        }
        let committed = self.execute(expected, command).await?;

        if committed.revision() == expected {
            return Ok(current);
        }
        let next = expected
            .checked_next()
            .expect("repository committed a revision after the persisted maximum");
        assert_eq!(
            committed.revision(),
            next,
            "repository committed an unexpected configuration revision"
        );
        let (_, proxies) = committed.into_parts();
        let snapshot = PublishedSnapshot::new(next, proxies);

        let activation = self.runtime.reconcile_configuration();
        let published = self.snapshots.replace(snapshot);
        activation.notify_after_snapshot_swap();
        Ok(published)
    }

    async fn execute(
        &self,
        expected: ConfigRevision,
        command: ProxyCommand,
    ) -> Result<StoredConfiguration, ConfigPublishError> {
        let result = match command {
            ProxyCommand::Create { id, draft } => {
                self.repository.create_proxy(expected, id, draft).await
            }
            ProxyCommand::Update { id, draft } => {
                self.repository.update_proxy(expected, id, draft).await
            }
            ProxyCommand::Delete { id } => self.repository.delete_proxy(expected, id).await,
            ProxyCommand::SetGlobal { id } => self.repository.set_global_proxy(expected, id).await,
        };

        result.map_err(ConfigPublishError::from)
    }
}

enum ProxyCommand {
    Create {
        id: ProxyProfileId,
        draft: ProxyDraft,
    },
    Update {
        id: ProxyProfileId,
        draft: ProxyDraft,
    },
    Delete {
        id: ProxyProfileId,
    },
    SetGlobal {
        id: ProxyProfileId,
    },
}

#[derive(Debug, Error)]
pub enum ConfigPublishError {
    #[error("configuration revision conflict")]
    RevisionConflict {
        expected: ConfigRevision,
        actual: ConfigRevision,
    },
    #[error("configuration revision cannot be incremented")]
    RevisionOverflow,
    #[error("proxy profile was not found")]
    ProxyNotFound,
    #[error("the built-in DIRECT proxy cannot be changed")]
    ProxyProtected,
    #[error("proxy profile is currently selected as the global proxy")]
    ProxyInUse,
    #[error("disabled proxy profile cannot be selected as global")]
    ProxyDisabled,
    #[error("proxy name is already in use")]
    ProxyNameConflict,
    #[error("proxy configuration is invalid: {0}")]
    InvalidProxy(ProxyValidationError),
    #[error("configuration storage failed")]
    Internal(#[source] StorageError),
}

impl From<StorageError> for ConfigPublishError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::RevisionConflict { expected, actual } => {
                Self::RevisionConflict { expected, actual }
            }
            StorageError::RevisionOverflow => Self::RevisionOverflow,
            StorageError::ProxyNotFound(_) => Self::ProxyNotFound,
            StorageError::ProxyProtected => Self::ProxyProtected,
            StorageError::ProxyInUse => Self::ProxyInUse,
            StorageError::ProxyDisabled => Self::ProxyDisabled,
            StorageError::ProxyNameConflict => Self::ProxyNameConflict,
            StorageError::ProxyValidation(error) => Self::InvalidProxy(error),
            other => Self::Internal(other),
        }
    }
}

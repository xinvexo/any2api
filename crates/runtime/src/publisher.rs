use std::sync::Arc;

use any2api_domain::ConfigRevision;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::published_snapshot::{PublishedSnapshot, SnapshotStore};

#[derive(Debug)]
pub struct PreparedPublish {
    snapshot: PublishedSnapshot,
}

impl PreparedPublish {
    #[must_use]
    pub const fn new(snapshot: PublishedSnapshot) -> Self {
        Self { snapshot }
    }
}

#[derive(Debug)]
pub struct ConfigPublisher {
    snapshots: Arc<SnapshotStore>,
    serial: Mutex<()>,
}

impl ConfigPublisher {
    #[must_use]
    pub fn new(snapshots: Arc<SnapshotStore>) -> Self {
        Self {
            snapshots,
            serial: Mutex::new(()),
        }
    }

    pub async fn activate_committed(
        &self,
        prepared: PreparedPublish,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.serial.lock().await;
        let current = self.snapshots.load();
        let current_revision = current.revision();
        let next_revision = prepared.snapshot.revision();

        if next_revision <= current_revision {
            return Err(ConfigPublishError::NonMonotonicRevision {
                current: current_revision,
                next: next_revision,
            });
        }

        Ok(self.snapshots.swap(prepared.snapshot))
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ConfigPublishError {
    #[error("configuration revision must increase: current={current:?}, next={next:?}")]
    NonMonotonicRevision {
        current: ConfigRevision,
        next: ConfigRevision,
    },
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use any2api_domain::ConfigRevision;

    use super::{ConfigPublishError, ConfigPublisher, PreparedPublish};
    use crate::published_snapshot::{PublishedSnapshot, SnapshotStore};

    #[tokio::test]
    async fn committed_revisions_only_move_forward() {
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            ConfigRevision::INITIAL,
        )));
        let publisher = ConfigPublisher::new(Arc::clone(&snapshots));
        let revision_two = ConfigRevision::INITIAL
            .checked_next()
            .expect("revision two");

        publisher
            .activate_committed(PreparedPublish::new(PublishedSnapshot::new(revision_two)))
            .await
            .expect("new revision activates");

        let error = publisher
            .activate_committed(PreparedPublish::new(PublishedSnapshot::new(revision_two)))
            .await
            .expect_err("duplicate revision must fail");

        assert_eq!(
            error,
            ConfigPublishError::NonMonotonicRevision {
                current: revision_two,
                next: revision_two,
            }
        );
        assert_eq!(snapshots.load().revision(), revision_two);
    }
}

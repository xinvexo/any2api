use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use any2api_domain::ConfigRevision;
use arc_swap::ArcSwap;
use tokio::sync::{Mutex, MutexGuard, watch};

use super::PublishedSnapshot;

#[derive(Debug)]
pub struct SnapshotStore {
    current: ArcSwap<PublishedSnapshot>,
    publish_serial: Mutex<()>,
    revision_sender: watch::Sender<ConfigRevision>,
    latest_operator_revision: AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PublicationSource {
    Operator,
    AutomaticOAuthRefresh,
}

impl SnapshotStore {
    #[must_use]
    pub fn new(initial: PublishedSnapshot) -> Self {
        let (revision_sender, _receiver) = watch::channel(initial.revision());
        let initial_revision = initial.revision();
        Self {
            current: ArcSwap::from_pointee(initial),
            publish_serial: Mutex::new(()),
            revision_sender,
            latest_operator_revision: AtomicU64::new(initial_revision.get()),
        }
    }

    #[must_use]
    pub fn load(&self) -> Arc<PublishedSnapshot> {
        self.current.load_full()
    }

    pub(crate) async fn acquire_publish(&self) -> MutexGuard<'_, ()> {
        self.publish_serial.lock().await
    }

    pub(crate) fn can_rebase_after_automatic_publications(
        &self,
        expected: ConfigRevision,
        actual: ConfigRevision,
    ) -> bool {
        let latest_operator = self.latest_operator_revision();
        expected < actual && expected >= latest_operator && actual >= latest_operator
    }

    pub(crate) fn subscribe_revision(&self) -> watch::Receiver<ConfigRevision> {
        self.revision_sender.subscribe()
    }

    pub(crate) fn replace(
        &self,
        next: PublishedSnapshot,
        source: PublicationSource,
    ) -> Arc<PublishedSnapshot> {
        let next = Arc::new(next);
        self.current.store(Arc::clone(&next));
        if source == PublicationSource::Operator {
            self.latest_operator_revision
                .store(next.revision().get(), Ordering::Release);
        }
        self.revision_sender.send_replace(next.revision());
        next
    }

    fn latest_operator_revision(&self) -> ConfigRevision {
        ConfigRevision::new(self.latest_operator_revision.load(Ordering::Acquire))
            .expect("operator revision is always initialized from a valid snapshot")
    }
}

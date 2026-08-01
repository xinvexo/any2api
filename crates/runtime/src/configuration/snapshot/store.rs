use std::sync::Arc;

use any2api_domain::ConfigRevision;
use arc_swap::ArcSwap;
use tokio::sync::{Mutex, MutexGuard, watch};

use super::PublishedSnapshot;

#[derive(Debug)]
pub struct SnapshotStore {
    current: ArcSwap<PublishedSnapshot>,
    publish_serial: Mutex<()>,
    revision_sender: watch::Sender<ConfigRevision>,
}

impl SnapshotStore {
    #[must_use]
    pub fn new(initial: PublishedSnapshot) -> Self {
        let (revision_sender, _receiver) = watch::channel(initial.revision());
        Self {
            current: ArcSwap::from_pointee(initial),
            publish_serial: Mutex::new(()),
            revision_sender,
        }
    }

    #[must_use]
    pub fn load(&self) -> Arc<PublishedSnapshot> {
        self.current.load_full()
    }

    pub(crate) async fn acquire_publish(&self) -> MutexGuard<'_, ()> {
        self.publish_serial.lock().await
    }

    pub(crate) fn subscribe_revision(&self) -> watch::Receiver<ConfigRevision> {
        self.revision_sender.subscribe()
    }

    pub(crate) fn replace(&self, next: PublishedSnapshot) -> Arc<PublishedSnapshot> {
        let next = Arc::new(next);
        self.current.store(Arc::clone(&next));
        self.revision_sender.send_replace(next.revision());
        next
    }
}

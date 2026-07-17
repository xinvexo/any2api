use std::sync::Arc;

use any2api_domain::ConfigRevision;
use arc_swap::ArcSwap;

#[derive(Debug)]
pub struct PublishedSnapshot {
    revision: ConfigRevision,
}

impl PublishedSnapshot {
    #[must_use]
    pub const fn new(revision: ConfigRevision) -> Self {
        Self { revision }
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }
}

#[derive(Debug)]
pub struct SnapshotStore {
    current: ArcSwap<PublishedSnapshot>,
}

impl SnapshotStore {
    #[must_use]
    pub fn new(initial: PublishedSnapshot) -> Self {
        Self {
            current: ArcSwap::from_pointee(initial),
        }
    }

    #[must_use]
    pub fn load(&self) -> Arc<PublishedSnapshot> {
        self.current.load_full()
    }

    pub(crate) fn swap(&self, next: PublishedSnapshot) -> Arc<PublishedSnapshot> {
        self.current.swap(Arc::new(next))
    }
}

use std::sync::Arc;

use any2api_domain::{ConfigRevision, ProxyConfiguration};
use arc_swap::ArcSwap;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Debug)]
pub struct PublishedSnapshot {
    revision: ConfigRevision,
    proxies: ProxyConfiguration,
}

impl PublishedSnapshot {
    #[must_use]
    pub const fn new(revision: ConfigRevision, proxies: ProxyConfiguration) -> Self {
        Self { revision, proxies }
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    #[must_use]
    pub const fn proxies(&self) -> &ProxyConfiguration {
        &self.proxies
    }
}

#[derive(Debug)]
pub struct SnapshotStore {
    current: ArcSwap<PublishedSnapshot>,
    publish_serial: Mutex<()>,
}

impl SnapshotStore {
    #[must_use]
    pub fn new(initial: PublishedSnapshot) -> Self {
        Self {
            current: ArcSwap::from_pointee(initial),
            publish_serial: Mutex::new(()),
        }
    }

    #[must_use]
    pub fn load(&self) -> Arc<PublishedSnapshot> {
        self.current.load_full()
    }

    pub(crate) async fn acquire_publish(&self) -> MutexGuard<'_, ()> {
        self.publish_serial.lock().await
    }

    pub(crate) fn replace(&self, next: PublishedSnapshot) -> Arc<PublishedSnapshot> {
        let next = Arc::new(next);
        self.current.store(Arc::clone(&next));
        next
    }
}

use std::sync::Arc;

use any2api_runtime::api::{ConfigPublisher, RuntimeRegistry, SnapshotStore};

#[derive(Clone)]
pub struct AppState {
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: Arc<ConfigPublisher>,
}

impl AppState {
    #[must_use]
    pub fn new(
        snapshots: Arc<SnapshotStore>,
        runtime: Arc<RuntimeRegistry>,
        publisher: Arc<ConfigPublisher>,
    ) -> Self {
        Self {
            snapshots,
            runtime,
            publisher,
        }
    }

    #[must_use]
    pub fn snapshots(&self) -> &SnapshotStore {
        &self.snapshots
    }

    #[must_use]
    pub fn runtime(&self) -> &RuntimeRegistry {
        &self.runtime
    }

    #[must_use]
    pub fn publisher(&self) -> &ConfigPublisher {
        &self.publisher
    }
}

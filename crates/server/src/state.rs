use std::sync::Arc;

use any2api_runtime::api::{RuntimeRegistry, SnapshotStore};

#[derive(Clone, Debug)]
pub struct AppState {
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
}

impl AppState {
    #[must_use]
    pub fn new(snapshots: Arc<SnapshotStore>, runtime: Arc<RuntimeRegistry>) -> Self {
        Self { snapshots, runtime }
    }

    #[must_use]
    pub fn snapshots(&self) -> &SnapshotStore {
        &self.snapshots
    }

    #[must_use]
    pub fn runtime(&self) -> &RuntimeRegistry {
        &self.runtime
    }
}

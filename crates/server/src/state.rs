use std::sync::Arc;

use any2api_runtime::api::{ConfigPublisher, PublicRequestService, RuntimeRegistry, SnapshotStore};

#[derive(Clone)]
pub struct AppState {
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: Arc<ConfigPublisher>,
    public_requests: Option<Arc<PublicRequestService>>,
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
            public_requests: None,
        }
    }

    #[must_use]
    pub fn with_public_requests(mut self, public_requests: Arc<PublicRequestService>) -> Self {
        self.public_requests = Some(public_requests);
        self
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

    #[must_use]
    pub fn public_requests(&self) -> Option<&PublicRequestService> {
        self.public_requests.as_deref()
    }
}

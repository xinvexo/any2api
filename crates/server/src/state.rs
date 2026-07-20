use std::sync::Arc;

use any2api_runtime::api::{
    ConfigPublisher, PublicRequestService, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};

use crate::admin_auth::{AdminAuthService, AdminNetworkPolicy};

#[derive(Clone)]
pub struct AppState {
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: Arc<ConfigPublisher>,
    public_requests: Option<Arc<PublicRequestService>>,
    admin_auth: Option<Arc<AdminAuthService>>,
    admin_network: Arc<AdminNetworkPolicy>,
    request_telemetry: Arc<RequestTelemetry>,
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
            admin_auth: None,
            admin_network: Arc::new(AdminNetworkPolicy::default()),
            request_telemetry: Arc::new(RequestTelemetry::disabled()),
        }
    }

    #[must_use]
    pub fn with_public_requests(mut self, public_requests: Arc<PublicRequestService>) -> Self {
        self.public_requests = Some(public_requests);
        self
    }

    #[must_use]
    pub fn with_admin_auth(
        mut self,
        admin_auth: Arc<AdminAuthService>,
        admin_network: AdminNetworkPolicy,
    ) -> Self {
        self.admin_auth = Some(admin_auth);
        self.admin_network = Arc::new(admin_network);
        self
    }

    #[must_use]
    pub fn with_request_telemetry(mut self, telemetry: Arc<RequestTelemetry>) -> Self {
        self.request_telemetry = telemetry;
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

    #[must_use]
    pub fn admin_auth(&self) -> Option<&AdminAuthService> {
        self.admin_auth.as_deref()
    }

    #[must_use]
    pub fn admin_network(&self) -> &AdminNetworkPolicy {
        &self.admin_network
    }

    #[must_use]
    pub fn request_telemetry(&self) -> &RequestTelemetry {
        &self.request_telemetry
    }
}

use std::sync::Arc;

use any2api_runtime::api::{
    ConfigPublisher, ProviderCredentialTestService, ProxyTestService, PublicRequestService,
    RequestTelemetry, RuntimeRegistry, SnapshotStore,
};

use crate::admin_auth::{AdminAuthService, AdminNetworkPolicy};

#[derive(Clone)]
pub struct AppState {
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: Arc<ConfigPublisher>,
    public_requests: Arc<PublicRequestService>,
    proxy_tests: Option<Arc<ProxyTestService>>,
    provider_credential_tests: Option<Arc<ProviderCredentialTestService>>,
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
        public_requests: Arc<PublicRequestService>,
    ) -> Self {
        Self {
            snapshots,
            runtime,
            publisher,
            public_requests,
            proxy_tests: None,
            provider_credential_tests: None,
            admin_auth: None,
            admin_network: Arc::new(AdminNetworkPolicy::default()),
            request_telemetry: Arc::new(RequestTelemetry::disabled()),
        }
    }

    #[must_use]
    pub fn with_proxy_tests(mut self, proxy_tests: Arc<ProxyTestService>) -> Self {
        self.proxy_tests = Some(proxy_tests);
        self
    }

    #[must_use]
    pub fn with_provider_credential_tests(
        mut self,
        tests: Arc<ProviderCredentialTestService>,
    ) -> Self {
        self.provider_credential_tests = Some(tests);
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
    pub fn public_requests(&self) -> &PublicRequestService {
        self.public_requests.as_ref()
    }

    #[must_use]
    pub fn proxy_tests(&self) -> Option<&ProxyTestService> {
        self.proxy_tests.as_deref()
    }

    #[must_use]
    pub fn provider_credential_tests(&self) -> Option<&ProviderCredentialTestService> {
        self.provider_credential_tests.as_deref()
    }

    #[must_use]
    pub fn admin_auth(&self) -> Option<&AdminAuthService> {
        self.admin_auth.as_deref()
    }

    #[must_use]
    pub fn admin_auth_handle(&self) -> Option<Arc<AdminAuthService>> {
        self.admin_auth.clone()
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

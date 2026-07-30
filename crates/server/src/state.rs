use std::sync::Arc;

use any2api_runtime::api::{
    ConfigPublisher, OAuthService, ProviderCredentialTestService, ProxyTestService,
    PublicRequestService, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_updater::api::ApplicationUpdateService;

use crate::admin_auth::AdminAuthService;

#[derive(Clone)]
pub struct AppState {
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: Arc<ConfigPublisher>,
    public_requests: Arc<PublicRequestService>,
    oauth: Option<Arc<OAuthService>>,
    proxy_tests: Option<Arc<ProxyTestService>>,
    provider_credential_tests: Option<Arc<ProviderCredentialTestService>>,
    admin_auth: Option<Arc<AdminAuthService>>,
    request_telemetry: Arc<RequestTelemetry>,
    application_updates: Option<Arc<dyn ApplicationUpdateService>>,
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
            oauth: None,
            proxy_tests: None,
            provider_credential_tests: None,
            admin_auth: None,
            request_telemetry: Arc::new(RequestTelemetry::disabled()),
            application_updates: None,
        }
    }

    #[must_use]
    pub fn with_oauth(mut self, oauth: Arc<OAuthService>) -> Self {
        self.oauth = Some(oauth);
        self
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
    pub fn with_admin_auth(mut self, admin_auth: Arc<AdminAuthService>) -> Self {
        self.admin_auth = Some(admin_auth);
        self
    }

    #[must_use]
    pub fn with_request_telemetry(mut self, telemetry: Arc<RequestTelemetry>) -> Self {
        self.request_telemetry = telemetry;
        self
    }

    #[must_use]
    pub fn with_application_updates(mut self, updates: Arc<dyn ApplicationUpdateService>) -> Self {
        self.application_updates = Some(updates);
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
    pub fn oauth(&self) -> Option<&OAuthService> {
        self.oauth.as_deref()
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
    pub fn request_telemetry(&self) -> &RequestTelemetry {
        &self.request_telemetry
    }

    #[must_use]
    pub(crate) fn request_telemetry_handle(&self) -> Arc<RequestTelemetry> {
        Arc::clone(&self.request_telemetry)
    }

    #[must_use]
    pub fn application_updates(&self) -> Option<&dyn ApplicationUpdateService> {
        self.application_updates.as_deref()
    }
}

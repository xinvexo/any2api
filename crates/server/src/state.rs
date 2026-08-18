use std::sync::{Arc, OnceLock};

use any2api_runtime::api::{
    ConfigPublisher, OAuthService, ProviderCredentialTestService, ProxyTestService,
    PublicRequestService, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_updater::api::ApplicationUpdateService;

use crate::{
    admin::realtime::AdminRealtimeHub, admin_auth::AdminAuthService, zstd_decode::ZstdDecoder,
};

#[derive(Clone)]
pub struct AppState {
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: Arc<ConfigPublisher>,
    public_requests: Arc<PublicRequestService>,
    oauth: Option<Arc<OAuthService>>,
    proxy_tests: Option<Arc<ProxyTestService>>,
    provider_credential_tests: Option<Arc<ProviderCredentialTestService>>,
    admin_auth: Arc<AdminAuthService>,
    request_telemetry: Arc<RequestTelemetry>,
    admin_realtime: Arc<OnceLock<Arc<AdminRealtimeHub>>>,
    application_updates: Option<Arc<dyn ApplicationUpdateService>>,
    zstd_decoder: ZstdDecoder,
}

impl AppState {
    #[must_use]
    pub fn new(
        snapshots: Arc<SnapshotStore>,
        runtime: Arc<RuntimeRegistry>,
        publisher: Arc<ConfigPublisher>,
        public_requests: Arc<PublicRequestService>,
        admin_auth: Arc<AdminAuthService>,
    ) -> Self {
        let zstd_decoder = ZstdDecoder::new(runtime.lifecycle());
        Self {
            snapshots,
            runtime,
            publisher,
            public_requests,
            oauth: None,
            proxy_tests: None,
            provider_credential_tests: None,
            admin_auth,
            request_telemetry: Arc::new(RequestTelemetry::disabled()),
            admin_realtime: Arc::new(OnceLock::new()),
            application_updates: None,
            zstd_decoder,
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
    pub(crate) fn snapshots_handle(&self) -> Arc<SnapshotStore> {
        Arc::clone(&self.snapshots)
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
    pub fn admin_auth(&self) -> &AdminAuthService {
        self.admin_auth.as_ref()
    }

    #[must_use]
    pub fn admin_auth_handle(&self) -> Arc<AdminAuthService> {
        Arc::clone(&self.admin_auth)
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
    pub(crate) fn admin_realtime(&self) -> Arc<AdminRealtimeHub> {
        Arc::clone(self.admin_realtime.get_or_init(|| {
            Arc::new(AdminRealtimeHub::new(
                Arc::clone(&self.runtime),
                Arc::clone(&self.snapshots),
                Arc::clone(&self.public_requests),
                Arc::clone(&self.request_telemetry),
            ))
        }))
    }

    #[must_use]
    pub fn application_updates(&self) -> Option<&dyn ApplicationUpdateService> {
        self.application_updates.as_deref()
    }

    #[must_use]
    pub(crate) fn zstd_decoder(&self) -> &ZstdDecoder {
        &self.zstd_decoder
    }
}

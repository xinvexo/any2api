use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use any2api_domain::{
    CredentialId, CredentialKind, GatewayApiKeyDraft, ProtocolDialect, ProtocolOperation,
    ProviderCredentialDraft, ProviderCredentialModel, ProviderEndpointDraft, ProviderEndpointId,
    ProviderKind, ProxyProfileId, RequestId, RequestsPerMinute, SettingKey, SettingValue,
};
use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiImagesAdapter,
    OpenAiResponsesAdapter, api::ProtocolRegistry,
};
use any2api_provider::{CodexDriver, api::ProviderRegistry};
use any2api_runtime::api::{
    ConfigPublisher, GatewayApiKeyAuthProof, ProviderApiKeySecret, PublicRequest,
    PublicRequestService, PublicResponse, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{
    HeaderMap, HeaderValue, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use tempfile::TempDir;
use tokio::{sync::Semaphore, task::JoinHandle};

#[tokio::test]
async fn enabled_candidate_wakes_pending_balanced_session_without_creating_affinity() {
    let mut fixture = LiveRoutingFixture::new(false, true).await;
    let captured = fixture.snapshots.load();
    let first = fixture.spawn_request(Arc::clone(&captured), "balanced-session");
    fixture.transport.wait_for_first_call().await;
    let second = fixture.spawn_request(captured, "balanced-session");
    wait_until_waiting(fixture.runtime.as_ref(), 1).await;

    fixture.enable_standby_with_affinity_setting_change().await;
    let second = tokio::time::timeout(Duration::from_secs(2), second)
        .await
        .expect("enabled route must wake the pending request")
        .expect("pending request task");
    assert_eq!(second.status, StatusCode::OK, "{}", buffered_body(&second));
    assert_eq!(fixture.transport.calls(), 2);
    assert_eq!(
        fixture.transport.authorizations(),
        vec!["Bearer sk-primary", "Bearer sk-standby"]
    );
    let latest = fixture.snapshots.load();
    let affinity = fixture.runtime.affinity_snapshot(latest.affinity_policy());
    assert_eq!(affinity.active_session_count(), 0);
    assert_eq!(affinity.creating_session_count(), 0);

    fixture.transport.release_first();
    let first = first.await.expect("first request task");
    assert_eq!(first.status, StatusCode::OK, "{}", buffered_body(&first));
    assert_eq!(fixture.runtime.queue_waiting_count(), 0);
}

fn buffered_body(response: &PublicResponse) -> String {
    match &response.body {
        any2api_runtime::api::PublicResponseBody::Buffered(body) => {
            String::from_utf8_lossy(body).into_owned()
        }
        any2api_runtime::api::PublicResponseBody::Streaming(_) => "<streaming>".to_owned(),
    }
}

#[tokio::test]
async fn live_rebase_never_moves_an_existing_sticky_binding_to_a_new_candidate() {
    let mut fixture = LiveRoutingFixture::new(true, false).await;
    let captured = fixture.snapshots.load();
    let first = fixture
        .execute_request(Arc::clone(&captured), "sticky-session")
        .await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(fixture.transport.calls(), 1);
    assert_eq!(
        fixture
            .runtime
            .affinity_snapshot(captured.affinity_policy())
            .active_session_count(),
        1
    );

    let mut pending = fixture.spawn_request(captured, "sticky-session");
    wait_until_waiting(fixture.runtime.as_ref(), 1).await;
    fixture.enable_standby().await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut pending)
            .await
            .is_err(),
        "a newly enabled credential must not steal a bound session"
    );
    assert_eq!(fixture.transport.calls(), 1);

    fixture.disable_primary().await;
    let blocked = tokio::time::timeout(Duration::from_secs(2), pending)
        .await
        .expect("revoked binding must wake promptly")
        .expect("pending sticky request task");
    assert_eq!(blocked.status, StatusCode::CONFLICT);
    assert_eq!(fixture.transport.calls(), 1);
    assert_eq!(
        fixture.transport.authorizations(),
        vec!["Bearer sk-primary"]
    );
    assert_eq!(fixture.runtime.queue_waiting_count(), 0);
}

#[tokio::test]
async fn invalidated_gateway_auth_stops_pending_work_but_not_started_work() {
    let mut fixture = LiveRoutingFixture::new(false, true).await;
    let captured = fixture.snapshots.load();
    let first = fixture.spawn_request(Arc::clone(&captured), "auth-first");
    fixture.transport.wait_for_first_call().await;
    let second = fixture.spawn_request(captured, "auth-second");
    wait_until_waiting(fixture.runtime.as_ref(), 1).await;

    fixture.disable_gateway_key().await;
    let second = tokio::time::timeout(Duration::from_secs(2), second)
        .await
        .expect("auth invalidation must wake pending work")
        .expect("pending request task");
    assert_eq!(second.status, StatusCode::UNAUTHORIZED);
    assert_eq!(fixture.transport.calls(), 1);

    fixture.transport.release_first();
    let first = first.await.expect("started request task");
    assert_eq!(first.status, StatusCode::OK);
}

struct LiveRoutingFixture {
    _directory: TempDir,
    runtime: Arc<RuntimeRegistry>,
    snapshots: Arc<SnapshotStore>,
    publisher: ConfigPublisher,
    service: Arc<PublicRequestService>,
    transport: Arc<RecordingTransport>,
    authentication: GatewayApiKeyAuthProof,
    primary_id: CredentialId,
    primary_config_version: u64,
    standby_id: CredentialId,
    standby_config_version: u64,
}

impl LiveRoutingFixture {
    async fn new(affinity_enabled: bool, block_first: bool) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("config.sqlite3"))
                .await
                .expect("storage"),
        );
        let providers = any2api_contract_tests::build_provider_registry();
        let configuration = storage.load_configuration().await.expect("configuration");
        let runtime = Arc::new(RuntimeRegistry::new());
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(configuration, runtime.as_ref(), providers.as_ref())
                .expect("initial snapshot"),
        ));
        let publisher = ConfigPublisher::new(
            storage,
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            any2api_contract_tests::build_configuration_capabilities(),
        )
        .expect("configuration publisher");

        let authentication =
            any2api_contract_tests::create_gateway_authentication(&publisher, snapshots.as_ref())
                .await;
        let mut current = authentication.snapshot;
        if affinity_enabled {
            current = publisher
                .set_setting_override(
                    current.revision(),
                    SettingKey::AffinityEnabled,
                    SettingValue::Boolean(true),
                )
                .await
                .expect("affinity setting");
        }
        let endpoint_id = ProviderEndpointId::new();
        current = publisher
            .create_provider_endpoint(
                current.revision(),
                endpoint_id,
                ProviderEndpointDraft::new(
                    "Live Routing Endpoint",
                    ProviderKind::Codex,
                    "https://api.example.com/v1",
                    ProtocolDialect::OpenAiResponses,
                    None,
                    true,
                )
                .expect("endpoint draft"),
            )
            .await
            .expect("endpoint");

        let primary_id = CredentialId::new();
        current = create_credential(
            &publisher,
            current,
            endpoint_id,
            primary_id,
            credential_draft(
                "Primary",
                true,
                Some(RequestsPerMinute::new(1).expect("RPM")),
            ),
            "sk-primary",
        )
        .await;
        let primary_config_version = current
            .provider_credentials()
            .get(primary_id)
            .expect("primary credential")
            .config_version();

        let standby_id = CredentialId::new();
        current = create_credential(
            &publisher,
            current,
            endpoint_id,
            standby_id,
            credential_draft("Standby", false, None),
            "sk-standby",
        )
        .await;
        let standby_config_version = current
            .provider_credentials()
            .get(standby_id)
            .expect("standby credential")
            .config_version();
        assert_eq!(current.revision(), snapshots.load().revision());

        let transport = Arc::new(RecordingTransport::new(block_first));
        let service = Arc::new(build_service(Arc::clone(&transport)));
        Self {
            _directory: directory,
            runtime,
            snapshots,
            publisher,
            service,
            transport,
            authentication: authentication.proof,
            primary_id,
            primary_config_version,
            standby_id,
            standby_config_version,
        }
    }

    fn spawn_request(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        session: &'static str,
    ) -> JoinHandle<PublicResponse> {
        let service = Arc::clone(&self.service);
        let snapshots = Arc::clone(&self.snapshots);
        let request = self.request(session);
        let authentication = self.authentication;
        tokio::spawn(async move {
            service
                .execute(snapshots, snapshot, authentication, request)
                .await
        })
    }

    async fn execute_request(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        session: &'static str,
    ) -> PublicResponse {
        self.service
            .execute(
                Arc::clone(&self.snapshots),
                snapshot,
                self.authentication,
                self.request(session),
            )
            .await
    }

    fn request(&self, session: &'static str) -> PublicRequest {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("x-any2api-session", HeaderValue::from_static(session));
        PublicRequest {
            request_id: RequestId::new(),
            client_ip: "127.0.0.1".parse().expect("client IP"),
            operation: ProtocolOperation::Responses,
            headers,
            body: Bytes::from_static(br#"{"model":"live-model","input":"hello"}"#),
        }
    }

    async fn enable_standby(&mut self) {
        let current = self.snapshots.load();
        let next = self
            .publisher
            .update_provider_credential(
                current.revision(),
                self.standby_id,
                self.standby_config_version,
                credential_draft("Standby", true, None),
            )
            .await
            .expect("enable standby");
        self.standby_config_version = next
            .provider_credentials()
            .get(self.standby_id)
            .expect("enabled standby")
            .config_version();
    }

    async fn enable_standby_with_affinity_setting_change(&mut self) {
        let current = self.snapshots.load();
        let changed = self
            .publisher
            .set_setting_override(
                current.revision(),
                SettingKey::AffinityEnabled,
                SettingValue::Boolean(true),
            )
            .await
            .expect("change affinity setting");
        let next = self
            .publisher
            .update_provider_credential(
                changed.revision(),
                self.standby_id,
                self.standby_config_version,
                credential_draft("Standby", true, None),
            )
            .await
            .expect("enable standby");
        self.standby_config_version = next
            .provider_credentials()
            .get(self.standby_id)
            .expect("enabled standby")
            .config_version();
    }

    async fn disable_primary(&mut self) {
        let current = self.snapshots.load();
        let next = self
            .publisher
            .update_provider_credential(
                current.revision(),
                self.primary_id,
                self.primary_config_version,
                credential_draft(
                    "Primary",
                    false,
                    Some(RequestsPerMinute::new(1).expect("RPM")),
                ),
            )
            .await
            .expect("disable primary");
        self.primary_config_version = next
            .provider_credentials()
            .get(self.primary_id)
            .expect("disabled primary")
            .config_version();
    }

    async fn disable_gateway_key(&mut self) {
        let current = self.snapshots.load();
        let key = current
            .gateway_api_keys()
            .get(self.authentication.id())
            .expect("Gateway key");
        self.publisher
            .update_gateway_api_key(
                current.revision(),
                self.authentication.id(),
                key.config_version(),
                GatewayApiKeyDraft::new("live routing", false).expect("Gateway key draft"),
            )
            .await
            .expect("disable Gateway key");
    }
}

async fn create_credential(
    publisher: &ConfigPublisher,
    current: Arc<PublishedSnapshot>,
    endpoint_id: ProviderEndpointId,
    id: CredentialId,
    draft: ProviderCredentialDraft,
    secret: &str,
) -> Arc<PublishedSnapshot> {
    let created = publisher
        .create_provider_credential(
            current.revision(),
            id,
            endpoint_id,
            draft,
            ProviderApiKeySecret::new(secret.to_owned()),
        )
        .await
        .expect("credential");
    publisher
        .set_provider_credential_models(
            created.revision(),
            id,
            1,
            vec![ProviderCredentialModel::new("live-model", None).expect("credential model")],
        )
        .await
        .expect("credential models")
}

fn credential_draft(
    label: &str,
    enabled: bool,
    rpm: Option<RequestsPerMinute>,
) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        rpm,
        enabled,
    )
    .expect("credential draft")
}

fn build_service(transport: Arc<RecordingTransport>) -> PublicRequestService {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("chat completions adapter");
    protocols
        .register(Arc::new(OpenAiImagesAdapter::new()))
        .expect("images adapter");
    protocols
        .register(Arc::new(AnthropicMessagesAdapter::new()))
        .expect("messages adapter");
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    PublicRequestService::new(Arc::new(protocols), Arc::new(providers), transport)
        .expect("public request service")
}

async fn wait_until_waiting(runtime: &RuntimeRegistry, expected: u32) {
    for _ in 0..10_000 {
        if runtime.queue_waiting_count() == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("queue did not reach the expected waiting count");
}

struct RecordingTransport {
    block_first: bool,
    first_started: Semaphore,
    release_first: Semaphore,
    calls: AtomicUsize,
    authorizations: Mutex<Vec<String>>,
}

impl RecordingTransport {
    fn new(block_first: bool) -> Self {
        Self {
            block_first,
            first_started: Semaphore::new(0),
            release_first: Semaphore::new(0),
            calls: AtomicUsize::new(0),
            authorizations: Mutex::new(Vec::new()),
        }
    }

    async fn wait_for_first_call(&self) {
        self.first_started
            .acquire()
            .await
            .expect("first call signal")
            .forget();
    }

    fn release_first(&self) {
        self.release_first.add_permits(1);
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }

    fn authorizations(&self) -> Vec<String> {
        self.authorizations
            .lock()
            .expect("authorization lock")
            .clone()
    }
}

#[async_trait]
impl TransportManager for RecordingTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        let authorization = request
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .expect("upstream authorization")
            .to_owned();
        self.authorizations
            .lock()
            .expect("authorization lock")
            .push(authorization);
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if self.block_first && call == 0 {
            self.first_started.add_permits(1);
            self.release_first
                .acquire()
                .await
                .expect("release signal")
                .forget();
        }
        let body: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from(format!(
            r#"{{"id":"live-response-{call}","model":"live-model","output":[]}}"#,
        )))]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

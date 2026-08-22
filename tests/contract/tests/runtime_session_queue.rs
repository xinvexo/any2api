use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use any2api_contract_tests::seed_official_client_versions;
use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProtocolOperation,
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
    PublicRequestService, PublicResponse, PublicResponseBody, PublishedSnapshot, RuntimeRegistry,
    SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;
use tempfile::{TempDir, tempdir};
use tokio::sync::Semaphore;

#[tokio::test]
async fn rpm_wait_has_no_creating_lease_and_only_the_reservation_winner_attempts() {
    let harness = Harness::new(61, Some(1)).await;
    tokio::time::pause();

    let prime = execute_request(
        Arc::clone(&harness.service),
        Arc::clone(&harness.snapshots),
        Arc::clone(&harness.snapshot),
        harness.authentication,
        None,
    )
    .await;
    assert_eq!(prime.status, StatusCode::OK);

    let first = spawn_session_request(&harness, "shared-session");
    let second = spawn_session_request(&harness, "shared-session");
    wait_until_queue_count(&harness.runtime, 2).await;
    assert_eq!(harness.transport.calls(), 1);
    assert_eq!(harness.affinity_snapshot().creating_session_count(), 0);

    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    harness.transport.wait_until_blocked().await;
    wait_until_creating_count(&harness, 1).await;
    assert_eq!(harness.transport.calls(), 2);
    assert_eq!(harness.runtime.queue_waiting_count(), 1);

    harness.transport.release_blocked();
    wait_until_active_session_count(&harness, 1).await;
    assert_eq!(harness.affinity_snapshot().creating_session_count(), 0);
    assert_eq!(harness.transport.calls(), 2);

    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    let first = first.await.expect("first session request");
    let second = second.await.expect("second session request");
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(harness.transport.calls(), 3);
    assert_eq!(harness.runtime.queue_waiting_count(), 0);
    assert_eq!(harness.affinity_snapshot().active_session_count(), 1);
}

#[tokio::test]
async fn rpm_queue_timeout_reports_the_scheduler_cause_not_binding_creation() {
    let harness = Harness::new(1, None).await;
    tokio::time::pause();

    let prime = execute_request(
        Arc::clone(&harness.service),
        Arc::clone(&harness.snapshots),
        Arc::clone(&harness.snapshot),
        harness.authentication,
        None,
    )
    .await;
    assert_eq!(prime.status, StatusCode::OK);

    let queued = spawn_session_request(&harness, "rate-timeout-session");
    wait_until_queue_count(&harness.runtime, 1).await;
    assert_eq!(harness.affinity_snapshot().creating_session_count(), 0);

    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    let response = queued.await.expect("queued session request");
    assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
    let body = buffered_json(response);
    assert_eq!(body["error"]["code"], "local_rate_limit");
    assert_eq!(
        body["error"]["message"],
        "all eligible credentials have exhausted their local RPM"
    );
    assert_ne!(
        body["error"]["message"],
        "session binding creation timed out"
    );
    assert_eq!(harness.transport.calls(), 1);
    assert_eq!(harness.runtime.queue_waiting_count(), 0);
    assert_eq!(harness.affinity_snapshot().creating_session_count(), 0);
}

struct Harness {
    _directory: TempDir,
    runtime: Arc<RuntimeRegistry>,
    snapshots: Arc<SnapshotStore>,
    snapshot: Arc<PublishedSnapshot>,
    authentication: GatewayApiKeyAuthProof,
    service: Arc<PublicRequestService>,
    transport: Arc<CallGateTransport>,
}

impl Harness {
    async fn new(queue_timeout_secs: u64, blocked_call: Option<usize>) -> Self {
        let directory = tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("config.sqlite3"))
                .await
                .expect("storage"),
        );
        let configuration = storage.load_configuration().await.expect("configuration");
        let runtime = Arc::new(RuntimeRegistry::new());
        let capabilities = any2api_contract_tests::build_configuration_capabilities();
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(
                configuration,
                runtime.as_ref(),
                capabilities.provider_registry(),
            )
            .expect("initial snapshot"),
        ));
        let publisher = ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            capabilities,
        )
        .expect("configuration publisher");
        let configured = publisher
            .set_setting_override(
                ConfigRevision::INITIAL,
                SettingKey::AffinityEnabled,
                SettingValue::Boolean(true),
            )
            .await
            .expect("affinity setting");
        let configured = publisher
            .set_setting_override(
                configured.revision(),
                SettingKey::SchedulerQueueTimeout,
                SettingValue::DurationSecs(queue_timeout_secs),
            )
            .await
            .expect("queue timeout setting");
        let configured = publisher
            .set_setting_override(
                configured.revision(),
                SettingKey::RetryPrecommitTotalBudget,
                SettingValue::DurationSecs(240),
            )
            .await
            .expect("precommit budget setting");
        let endpoint_id = ProviderEndpointId::new();
        let endpoint = publisher
            .create_provider_endpoint(
                configured.revision(),
                endpoint_id,
                ProviderEndpointDraft::new(
                    "Session queue endpoint",
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
        let credential_id = CredentialId::new();
        let credential = publisher
            .create_provider_credential(
                endpoint.revision(),
                credential_id,
                endpoint_id,
                ProviderCredentialDraft::new(
                    "Session queue credential",
                    CredentialKind::ApiKey,
                    ProxyProfileId::DIRECT,
                    Some(RequestsPerMinute::new(1).expect("valid RPM")),
                    true,
                )
                .expect("credential draft"),
                ProviderApiKeySecret::new("sk-session-queue-contract".to_owned()),
            )
            .await
            .expect("credential");
        publisher
            .set_provider_credential_models(
                credential.revision(),
                credential_id,
                1,
                vec![
                    ProviderCredentialModel::new("session-queued-model", None)
                        .expect("credential model"),
                ],
            )
            .await
            .expect("credential models");
        let authentication =
            any2api_contract_tests::create_gateway_authentication(&publisher, snapshots.as_ref())
                .await;
        let transport = Arc::new(CallGateTransport::new(blocked_call));
        let service = Arc::new(build_service(Arc::clone(&transport)));

        Self {
            _directory: directory,
            runtime,
            snapshots,
            snapshot: authentication.snapshot,
            authentication: authentication.proof,
            service,
            transport,
        }
    }

    fn affinity_snapshot(&self) -> any2api_runtime::api::AffinityRuntimeSnapshot {
        self.runtime
            .affinity_snapshot(self.snapshot.affinity_policy())
    }
}

fn build_service(transport: Arc<CallGateTransport>) -> PublicRequestService {
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
    seed_official_client_versions(&providers);
    PublicRequestService::new(Arc::new(protocols), Arc::new(providers), transport)
        .expect("public request service")
}

fn spawn_session_request(
    harness: &Harness,
    session: &'static str,
) -> tokio::task::JoinHandle<PublicResponse> {
    tokio::spawn(execute_request(
        Arc::clone(&harness.service),
        Arc::clone(&harness.snapshots),
        Arc::clone(&harness.snapshot),
        harness.authentication,
        Some(session),
    ))
}

async fn execute_request(
    service: Arc<PublicRequestService>,
    snapshots: Arc<SnapshotStore>,
    snapshot: Arc<PublishedSnapshot>,
    authentication: GatewayApiKeyAuthProof,
    session: Option<&'static str>,
) -> PublicResponse {
    let mut headers =
        HeaderMap::from_iter([(CONTENT_TYPE, HeaderValue::from_static("application/json"))]);
    if let Some(session) = session {
        headers.insert("session-id", HeaderValue::from_static(session));
    }
    service
        .execute(
            snapshots,
            snapshot,
            authentication,
            PublicRequest {
                request_id: RequestId::new(),
                client_ip: "127.0.0.1".parse().expect("client IP"),
                operation: ProtocolOperation::Responses,
                headers,
                body: Bytes::from_static(br#"{"model":"session-queued-model","input":"hello"}"#),
            },
        )
        .await
}

fn buffered_json(response: PublicResponse) -> Value {
    let PublicResponseBody::Buffered(body) = response.body else {
        panic!("expected a buffered response");
    };
    serde_json::from_slice(&body).expect("response JSON")
}

async fn wait_until_queue_count(runtime: &RuntimeRegistry, expected: u32) {
    for _ in 0..10_000 {
        if runtime.queue_waiting_count() == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("queue did not reach {expected}");
}

async fn wait_until_creating_count(harness: &Harness, expected: usize) {
    for _ in 0..10_000 {
        if harness.affinity_snapshot().creating_session_count() == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("creating count did not reach {expected}");
}

async fn wait_until_active_session_count(harness: &Harness, expected: usize) {
    for _ in 0..10_000 {
        if harness.affinity_snapshot().active_session_count() == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("active session count did not reach {expected}");
}

struct CallGateTransport {
    blocked_call: Option<usize>,
    blocked_started: Semaphore,
    release: Semaphore,
    calls: AtomicUsize,
}

impl CallGateTransport {
    fn new(blocked_call: Option<usize>) -> Self {
        Self {
            blocked_call,
            blocked_started: Semaphore::new(0),
            release: Semaphore::new(0),
            calls: AtomicUsize::new(0),
        }
    }

    async fn wait_until_blocked(&self) {
        self.blocked_started
            .acquire()
            .await
            .expect("blocked call signal")
            .forget();
    }

    fn release_blocked(&self) {
        self.release.add_permits(1);
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }
}

#[async_trait]
impl TransportManager for CallGateTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        _request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if self.blocked_call == Some(call) {
            self.blocked_started.add_permits(1);
            self.release
                .acquire()
                .await
                .expect("blocked call release")
                .forget();
        }
        let body = Bytes::from(format!(
            r#"{{"id":"session-queued-response-{call}","model":"session-queued-model","output":[]}}"#
        ));
        let body: BoxByteStream = Box::pin(stream::iter([Ok(body)]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

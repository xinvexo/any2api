use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use any2api_domain::{
    ConfigRevision, CredentialKind, GatewayApiKeyId, ProtocolDialect, ProtocolOperation,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId, RequestId, RequestsPerMinute, SettingKey, SettingValue,
};
use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiResponsesAdapter,
    ProtocolRegistry,
};
use any2api_provider::{CodexDriver, ProviderRegistry};
use any2api_runtime::api::{
    ConfigPublisher, ProviderApiKeySecret, PublicRequest, PublicRequestService, PublishedSnapshot,
    RuntimeRegistry, SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use tempfile::tempdir;
use tokio::sync::Semaphore;

#[tokio::test]
async fn rate_limited_request_waits_until_the_rolling_window_expires() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
        any2api_contract_tests::build_provider_registry().as_ref(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        any2api_contract_tests::build_configuration_capabilities(),
    )
    .expect("configuration publisher");
    let configured = publisher
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::SchedulerQueueTimeout,
            SettingValue::DurationSecs(61),
        )
        .await
        .expect("queue timeout setting");
    let configured = publisher
        .set_setting_override(
            configured.revision(),
            SettingKey::RetryPrecommitTotalBudget,
            SettingValue::DurationSecs(120),
        )
        .await
        .expect("precommit budget setting");
    let endpoint_id = ProviderEndpointId::new();
    let endpoint = publisher
        .create_provider_endpoint(
            configured.revision(),
            endpoint_id,
            ProviderEndpointDraft::new(
                "Queue Endpoint",
                ProviderKind::Codex,
                "https://api.example.com/v1",
                ProtocolDialect::OpenAiResponses,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("endpoint");
    let credential_id = any2api_domain::CredentialId::new();
    let credential = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            ProviderCredentialDraft::new(
                "Queue Credential",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                Some(RequestsPerMinute::new(1).expect("valid RPM")),
                true,
            )
            .expect("credential draft"),
            ProviderApiKeySecret::new("sk-queue-contract".to_owned()),
        )
        .await
        .expect("credential");
    let selected_models = publisher
        .set_provider_credential_models(
            credential.revision(),
            credential_id,
            1,
            vec!["queued-model".to_owned()],
        )
        .await
        .expect("credential models");

    tokio::time::pause();
    let transport = Arc::new(BlockingTransport::new());
    let service = Arc::new(build_service(transport.clone()));
    let first = tokio::spawn(execute_request(Arc::clone(&service), snapshots.load()));
    transport.wait_for_first_call().await;
    let second = tokio::spawn(execute_request(Arc::clone(&service), snapshots.load()));
    wait_until_waiting(&runtime, 1).await;
    assert_eq!(transport.calls(), 1);

    transport.release_first();
    let first_response = first.await.expect("first request task");
    assert_eq!(first_response.status, StatusCode::OK);
    assert!(!second.is_finished());
    assert_eq!(transport.calls(), 1);

    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    let second_response = second.await.expect("second request task");
    assert_eq!(second_response.status, StatusCode::OK);
    assert_eq!(transport.calls(), 2);
    assert_eq!(runtime.queue_waiting_count(), 0);
    assert_eq!(selected_models.revision(), snapshots.load().revision());
}

fn build_service(transport: Arc<BlockingTransport>) -> PublicRequestService {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("chat completions adapter");
    protocols
        .register(Arc::new(AnthropicMessagesAdapter::new()))
        .expect("messages adapter");
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("codex driver");
    PublicRequestService::new(Arc::new(protocols), Arc::new(providers), transport)
        .expect("public request service")
}

async fn execute_request(
    service: Arc<PublicRequestService>,
    snapshot: Arc<PublishedSnapshot>,
) -> any2api_runtime::api::PublicResponse {
    service
        .execute(
            snapshot,
            PublicRequest {
                request_id: RequestId::new(),
                gateway_api_key_id: GatewayApiKeyId::new(),
                operation: ProtocolOperation::Responses,
                headers: HeaderMap::from_iter([(
                    CONTENT_TYPE,
                    "application/json".parse().expect("content type"),
                )]),
                body: Bytes::from_static(br#"{"model":"queued-model","input":"hello"}"#),
            },
        )
        .await
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

struct BlockingTransport {
    first_started: Semaphore,
    release_first: Semaphore,
    calls: AtomicUsize,
}

impl BlockingTransport {
    fn new() -> Self {
        Self {
            first_started: Semaphore::new(0),
            release_first: Semaphore::new(0),
            calls: AtomicUsize::new(0),
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
}

#[async_trait]
impl TransportManager for BlockingTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        _request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if call == 0 {
            self.first_started.add_permits(1);
            self.release_first
                .acquire()
                .await
                .expect("release signal")
                .forget();
        }
        let body: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
            br#"{"id":"queued-response","model":"queued-model","output":[]}"#,
        ))]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

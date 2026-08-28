use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, GatewayApiKeyDraft, GatewayApiKeyId,
    ProtocolDialect, ProtocolOperation, ProviderCredentialDraft, ProviderCredentialModel,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyProfileId, RequestId,
};
use any2api_storage::api::{ConfigurationRepository, RequestLogRepository, SqliteStore};
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportFailureScope, TransportManager, TransportProxy,
    TransportRequest, TransportResponse,
};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures_util::{StreamExt, stream};
use http::{HeaderMap, HeaderValue, StatusCode, header};
use tempfile::{TempDir, tempdir};

use super::{PublicRequest, PublicRequestService, PublicResponseBody};
use crate::{
    configuration::{ConfigPublisher, PublishedSnapshot, SnapshotStore},
    credential::ProviderApiKeySecret,
    lifecycle::ProcessLifecycle,
    registry::RuntimeRegistry,
    request_telemetry::RequestTelemetry,
};

const MODEL: &str = "telemetry-execution-model";
const OFFICIAL_MESSAGE_SECRET_SENTINEL: &str = "upstream-secret-sentinel-0123456789";
const REQUEST_SECRET_SENTINEL: &str = "request-secret-sentinel-0123456789";

#[tokio::test]
async fn upstream_official_messages_are_forwarded_but_not_persisted() {
    for (script, streaming, expected_request_message) in [
        (Script::OfficialMessageHttpError, false, None),
        (
            Script::OfficialMessageStreamError,
            true,
            Some("upstream response stream reported a failure event"),
        ),
    ] {
        let transport = Arc::new(ScriptedTransport::new(script));
        let fixture = ExecutionFixture::new(transport, ProtocolDialect::OpenAiResponses).await;
        let request_id = RequestId::new();

        let response = fixture.execute_with_id(streaming, request_id).await;
        let body = drain_body(response.body).await;
        assert!(
            String::from_utf8_lossy(&body).contains(OFFICIAL_MESSAGE_SECRET_SENTINEL),
            "the upstream response itself must remain transparent"
        );

        let persisted = fixture.persisted_request(request_id).await;
        assert_eq!(
            persisted.request.error_message.as_deref(),
            expected_request_message
        );
        assert!(
            persisted
                .attempts
                .iter()
                .all(|attempt| attempt.error_message.is_none())
        );
        assert!(!persisted_request_contains(
            &persisted,
            OFFICIAL_MESSAGE_SECRET_SENTINEL
        ));
    }
}

#[tokio::test]
async fn bridged_tool_name_is_not_persisted_in_request_telemetry() {
    let transport = Arc::new(ScriptedTransport::new(Script::UnexpectedUpstream));
    let fixture =
        ExecutionFixture::new(transport.clone(), ProtocolDialect::OpenAiChatCompletions).await;
    let request_id = RequestId::new();

    let response = fixture
        .execute_payload(
            request_id,
            serde_json::json!({
                "model": MODEL,
                "input": "hello",
                "tools": [
                    {"type":"function","name":REQUEST_SECRET_SENTINEL},
                    {"type":"function","name":REQUEST_SECRET_SENTINEL}
                ]
            }),
        )
        .await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    let body = drain_body(response.body).await;
    assert!(!String::from_utf8_lossy(&body).contains(REQUEST_SECRET_SENTINEL));
    assert_eq!(transport.calls(), 0);

    let persisted = fixture.persisted_request(request_id).await;
    assert!(persisted.attempts.is_empty());
    assert!(!persisted_request_contains(
        &persisted,
        REQUEST_SECRET_SENTINEL
    ));
}

struct ExecutionFixture {
    service: PublicRequestService,
    snapshots: Arc<SnapshotStore>,
    snapshot: Arc<PublishedSnapshot>,
    authentication: crate::configuration::GatewayApiKeyAuthProof,
    repository: Arc<SqliteStore>,
    telemetry: Arc<RequestTelemetry>,
    _lifecycle: ProcessLifecycle,
    _directory: TempDir,
}

impl ExecutionFixture {
    async fn new(transport: Arc<dyn TransportManager>, endpoint_dialect: ProtocolDialect) -> Self {
        let directory = tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("telemetry-execution.sqlite3"))
                .await
                .expect("storage"),
        );
        let initial = storage.load_configuration().await.expect("configuration");
        let runtime = Arc::new(RuntimeRegistry::new());
        let capabilities = crate::test_support::configuration_capabilities();
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(initial, runtime.as_ref(), capabilities.provider_registry())
                .expect("initial snapshot"),
        ));
        let publisher = ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            runtime,
            capabilities,
        )
        .expect("configuration publisher");

        let endpoint_id = ProviderEndpointId::new();
        let endpoint = publisher
            .create_provider_endpoint(
                ConfigRevision::INITIAL,
                endpoint_id,
                ProviderEndpointDraft::new(
                    "Telemetry execution",
                    ProviderKind::OpenAi,
                    "https://api.example.com/v1",
                    endpoint_dialect,
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
                    "Telemetry execution",
                    CredentialKind::ApiKey,
                    ProxyProfileId::DIRECT,
                    None,
                    true,
                )
                .expect("credential draft"),
                ProviderApiKeySecret::new("sk-telemetry-execution".to_owned()),
            )
            .await
            .expect("credential");
        let configured = publisher
            .set_provider_credential_models(
                credential.revision(),
                credential_id,
                1,
                vec![ProviderCredentialModel::new(MODEL, None).expect("credential model")],
            )
            .await
            .expect("credential models");
        let gateway_key_id = GatewayApiKeyId::new();
        let published = publisher
            .create_gateway_api_key(
                configured.revision(),
                gateway_key_id,
                GatewayApiKeyDraft::new("Telemetry execution", true).expect("Gateway Key draft"),
            )
            .await
            .expect("Gateway Key");
        let snapshot = Arc::clone(published.snapshot());
        let token = published.token();
        let authentication = snapshot
            .authenticate_gateway_api_key(token)
            .expect("Gateway Key authentication");
        let lifecycle = ProcessLifecycle::new();
        let telemetry = Arc::new(RequestTelemetry::start(
            Arc::clone(&storage),
            snapshot.revision(),
            snapshot.settings().logging(),
            &lifecycle,
        ));
        let service = PublicRequestService::new(
            crate::test_support::protocol_registry(),
            crate::test_support::provider_registry(),
            transport,
        )
        .expect("public request service")
        .with_telemetry(Arc::clone(&telemetry));

        Self {
            service,
            snapshots,
            snapshot,
            authentication,
            repository: storage,
            telemetry,
            _lifecycle: lifecycle,
            _directory: directory,
        }
    }

    async fn execute_with_id(
        &self,
        streaming: bool,
        request_id: RequestId,
    ) -> super::PublicResponse {
        self.execute_payload(
            request_id,
            serde_json::json!({
                "model": MODEL,
                "input": "hello",
                "stream": streaming,
            }),
        )
        .await
    }

    async fn execute_payload(
        &self,
        request_id: RequestId,
        payload: serde_json::Value,
    ) -> super::PublicResponse {
        self.service
            .execute(
                Arc::clone(&self.snapshots),
                Arc::clone(&self.snapshot),
                self.authentication,
                PublicRequest {
                    request_id,
                    client_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    operation: ProtocolOperation::Responses,
                    headers: HeaderMap::new(),
                    body: Bytes::from(serde_json::to_vec(&payload).expect("request JSON")),
                },
            )
            .await
    }

    async fn persisted_request(
        &self,
        request_id: RequestId,
    ) -> any2api_domain::CompletedRequestLog {
        self.telemetry
            .shutdown(std::time::Duration::from_secs(5))
            .await;
        self.repository
            .get_request_log(request_id)
            .await
            .expect("request log query")
            .expect("persisted request log")
    }
}

#[derive(Clone, Copy)]
enum Script {
    UnexpectedUpstream,
    OfficialMessageHttpError,
    OfficialMessageStreamError,
}

struct ScriptedTransport {
    script: Script,
    calls: AtomicUsize,
}

impl ScriptedTransport {
    const fn new(script: Script) -> Self {
        Self {
            script,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }
}

#[async_trait]
impl TransportManager for ScriptedTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        _request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        let (status, content_type, body) = match self.script {
            Script::UnexpectedUpstream => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "application/json",
                Bytes::from_static(
                    br#"{"error":{"type":"server_error","code":"server_error","message":"failed"}}"#,
                ),
            ),
            Script::OfficialMessageHttpError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "application/json",
                Bytes::from(format!(
                    r#"{{"error":{{"type":"server_error","code":"server_error","message":"{OFFICIAL_MESSAGE_SECRET_SENTINEL}"}}}}"#
                )),
            ),
            Script::OfficialMessageStreamError => (
                StatusCode::OK,
                "text/event-stream",
                Bytes::from(format!(
                    "event: response.output_text.delta\ndata: {{\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}}\n\nevent: error\ndata: {{\"type\":\"error\",\"error\":{{\"type\":\"server_error\",\"code\":\"server_error\",\"message\":\"{OFFICIAL_MESSAGE_SECRET_SENTINEL}\"}}}}\n\n"
                )),
            ),
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        let body: BoxByteStream = Box::pin(stream::iter([Ok(body)]));
        Ok(TransportResponse {
            status,
            headers,
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

fn persisted_request_contains(
    record: &any2api_domain::CompletedRequestLog,
    sentinel: &str,
) -> bool {
    record
        .request
        .error_message
        .as_deref()
        .is_some_and(|message| message.contains(sentinel))
        || record.attempts.iter().any(|attempt| {
            attempt
                .error_message
                .as_deref()
                .is_some_and(|message| message.contains(sentinel))
        })
}

async fn drain_body(body: PublicResponseBody) -> Bytes {
    match body {
        PublicResponseBody::Buffered(body) => body,
        PublicResponseBody::Streaming(mut stream) => {
            let mut body = BytesMut::new();
            while let Some(chunk) = stream.next().await {
                body.extend_from_slice(&chunk.expect("stream chunk"));
            }
            body.freeze()
        }
    }
}

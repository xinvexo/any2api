use std::sync::{Arc, Mutex};

use any2api_domain::{
    ConfigRevision, GatewayApiKeyId, OAuthAccountDraft, OAuthAccountId, ProtocolOperation,
    ProviderKind, ProxyProfileId, RequestId,
};
use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiImagesAdapter,
    OpenAiResponsesAdapter, ProtocolRegistry, ResponsesToChatCompletionsBridge,
};
use any2api_provider::{ClaudeDriver, CodexDriver, api::ProviderRegistry};
use any2api_runtime::api::{
    PublicRequest, PublicRequestService, PublicResponseBody, PublishedSnapshot, RequestTelemetry,
    RuntimeRegistry,
};
use any2api_storage::api::{
    ConfigurationMutation, ConfigurationRepository, OAuthAccountDocument, SqliteStore,
    StoredConfiguration,
};
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use tempfile::tempdir;

#[tokio::test]
async fn codex_oauth_account_uses_fixed_route_shared_permit_and_distinct_log_source() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("oauth-routing.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let account_id = OAuthAccountId::new();
    let configuration = commit_configuration(
        storage.as_ref(),
        initial.revision(),
        ConfigurationMutation::CreateOAuthAccount {
            id: account_id,
            provider_kind: ProviderKind::Codex,
            draft: OAuthAccountDraft::new("Codex OAuth", None, true)
                .expect("OAuth account draft"),
            safe_account_email: Some("person@example.com".into()),
            expires_at: None,
            models: vec!["gpt-5.5".into()],
            document: OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"access_token":"oauth-access-secret","refresh_token":null,"id_token":null,"account_id":"account-123","email":"person@example.com"}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth document"),
        },
    )
    .await;

    let providers = providers();
    let protocols = protocols();
    let runtime = Arc::new(RuntimeRegistry::new());
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        configuration.revision(),
        configuration.settings().logging(),
        &runtime.lifecycle(),
    ));
    let snapshot = Arc::new(
        PublishedSnapshot::new(configuration, runtime.as_ref(), providers.as_ref())
            .expect("initial snapshot"),
    );
    let transport = Arc::new(CapturingTransport::default());
    let service = PublicRequestService::new(
        protocols,
        Arc::clone(&providers),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
    )
    .expect("public request service")
    .with_telemetry(Arc::clone(&telemetry));
    let request_id = RequestId::new();

    let response = service
        .execute(
            Arc::clone(&snapshot),
            PublicRequest {
                request_id,
                gateway_api_key_id: GatewayApiKeyId::new(),
                client_ip: "127.0.0.1".parse().expect("client IP"),
                operation: ProtocolOperation::Responses,
                headers: HeaderMap::new(),
                body: Bytes::from(
                    serde_json::to_vec(&serde_json::json!({
                        "model": "gpt-5.5",
                        "input": [{
                            "type": "reasoning",
                            "id": "item_oauth_incompatible",
                            "summary": [],
                            "encrypted_content": "opaque-reasoning"
                        }]
                    }))
                    .expect("request JSON"),
                ),
            },
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert!(matches!(response.body, PublicResponseBody::Buffered(_)));
    assert!(snapshot.public_model_names().contains("gpt-5.5"));
    assert_eq!(runtime.active_credential_count(), 1);
    let captured = transport.take();
    assert_eq!(
        captured.request.uri.to_string(),
        "https://chatgpt.com/backend-api/codex/responses"
    );
    assert_eq!(
        captured.request.headers[AUTHORIZATION],
        "Bearer oauth-access-secret"
    );
    assert_eq!(
        captured.request.headers["chatgpt-account-id"],
        "account-123"
    );
    assert_eq!(captured.request.headers["originator"], "codex_cli_rs");
    assert_eq!(captured.proxy_id, ProxyProfileId::DIRECT);
    let outbound: serde_json::Value =
        serde_json::from_slice(&captured.request.body).expect("outbound JSON");
    assert_eq!(outbound["model"], "gpt-5.5");
    assert!(outbound["input"][0].get("id").is_none());
    assert_eq!(
        outbound["input"][0]["encrypted_content"],
        "opaque-reasoning"
    );

    let log = wait_for_log(telemetry.as_ref(), request_id).await;
    assert_eq!(log.request.credential_id, None);
    assert_eq!(log.request.oauth_account_id, Some(account_id));
    assert_eq!(log.request.provider_endpoint_id, None);
    assert_eq!(log.attempts.len(), 1);
    assert_eq!(log.attempts[0].credential_id, None);
    assert_eq!(log.attempts[0].oauth_account_id, Some(account_id));
}

async fn commit_configuration(
    storage: &SqliteStore,
    expected: ConfigRevision,
    mutation: ConfigurationMutation,
) -> StoredConfiguration {
    use std::convert::Infallible;

    use any2api_storage::api::{
        ConfigurationTransactionOutcome, ConfigurationTransactionRepository,
    };

    let outcome = <SqliteStore as ConfigurationTransactionRepository<
        StoredConfiguration,
        Infallible,
    >>::transact_configuration(storage, expected, mutation, Box::new(Ok))
    .await
    .expect("commit configuration");
    match outcome {
        ConfigurationTransactionOutcome::NoChange => storage
            .load_configuration()
            .await
            .expect("load unchanged configuration"),
        ConfigurationTransactionOutcome::Committed(configuration) => configuration,
        ConfigurationTransactionOutcome::Rejected(never) => match never {},
    }
}

fn providers() -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    providers
        .register(Arc::new(ClaudeDriver::new()))
        .expect("Claude driver");
    Arc::new(providers)
}

fn protocols() -> Arc<ProtocolRegistry> {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat Completions adapter");
    protocols
        .register(Arc::new(OpenAiImagesAdapter::new()))
        .expect("Images adapter");
    protocols
        .register(Arc::new(AnthropicMessagesAdapter::new()))
        .expect("Messages adapter");
    protocols
        .register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))
        .expect("Responses bridge");
    Arc::new(protocols)
}

async fn wait_for_log(
    telemetry: &RequestTelemetry,
    request_id: RequestId,
) -> any2api_domain::CompletedRequestLog {
    for _ in 0..200 {
        if let Some(record) = telemetry.get(request_id).await.expect("request log") {
            return record;
        }
        tokio::task::yield_now().await;
    }
    panic!("request log was not persisted");
}

struct CapturedRequest {
    proxy_id: ProxyProfileId,
    request: TransportRequest,
}

#[derive(Default)]
struct CapturingTransport {
    captured: Mutex<Option<CapturedRequest>>,
}

impl CapturingTransport {
    fn take(&self) -> CapturedRequest {
        self.captured
            .lock()
            .expect("captured request lock")
            .take()
            .expect("captured request")
    }
}

#[async_trait]
impl TransportManager for CapturingTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        *self.captured.lock().expect("captured request lock") = Some(CapturedRequest {
            proxy_id: proxy.profile().id(),
            request,
        });
        let body: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
            br#"{"id":"resp_oauth","object":"response","status":"completed","model":"gpt-5.5","output":[]}"#,
        ))]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

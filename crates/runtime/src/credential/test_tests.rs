use std::sync::{Arc, Mutex};

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyProfileId, RequestsPerMinute,
    RetrySafety, UpstreamErrorClassification, UpstreamErrorKind,
};
use any2api_provider::{CodexDriver, ProviderRegistry};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    TransportFailureScope, TransportManager, TransportProxy, TransportRequest, TransportResponse,
};
use async_trait::async_trait;
use http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigPublisher, PublishedSnapshot, SnapshotStore},
    credential::{
        ProviderApiKeySecret, ProviderCredentialTestOutcome, ProviderCredentialTestService,
    },
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn accepted_probe_uses_current_secret_and_clears_only_its_generation_auth_error() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(
            configuration,
            runtime.as_ref(),
            crate::test_support::configuration_capabilities().provider_registry(),
        )
        .expect("initial snapshot"),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, endpoint_draft(true))
        .await
        .expect("endpoint");
    let snapshot = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(true),
            ProviderApiKeySecret::new("sk-probe-current".into()),
        )
        .await
        .expect("credential");
    let binding = snapshot
        .credential_runtime(credential_id.into())
        .expect("credential runtime");
    binding.generation().health().record(
        "model",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::Authentication,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &crate::health::ReliabilityPolicy::from_settings(snapshot.settings().reliability()),
    );
    assert!(binding.generation().health().has_auth_error());
    let epoch_before = runtime.scheduler_epoch();

    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    let transport = Arc::new(CapturingTransport::default());
    let service = ProviderCredentialTestService::new(
        Arc::new(providers),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
    );

    let result = service
        .test(Arc::clone(&snapshot), credential_id)
        .await
        .expect("credential test");

    match &result.outcome {
        ProviderCredentialTestOutcome::Accepted {
            status_code: 200,
            auth_error_cleared: true,
            models,
        } => assert_eq!(models, &["gpt-probe"]),
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(!binding.generation().health().has_auth_error());
    assert!(runtime.scheduler_epoch() > epoch_before);
    assert_eq!(binding.in_flight(), 0);
    assert_eq!(binding.rate_snapshot().requests_in_window(), 1);
    let captured = transport.request.lock().expect("captured request");
    let request = captured.as_ref().expect("probe request");
    assert_eq!(request.uri.path(), "/v1/models");
    assert_eq!(request.headers[AUTHORIZATION], "Bearer sk-probe-current");
}

#[tokio::test]
async fn probe_ignores_credential_and_endpoint_routing_enabled_state() {
    for (endpoint_enabled, credential_enabled) in [(false, true), (true, false), (false, false)] {
        assert_probe_available(endpoint_enabled, credential_enabled).await;
    }
}

async fn assert_probe_available(endpoint_enabled: bool, credential_enabled: bool) {
    use any2api_provider::WorkBuddyDriver;

    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(
            configuration,
            runtime.as_ref(),
            crate::test_support::configuration_capabilities().provider_registry(),
        )
        .expect("initial snapshot"),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        runtime,
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            endpoint_draft(endpoint_enabled),
        )
        .await
        .expect("endpoint");
    let snapshot = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(credential_enabled),
            ProviderApiKeySecret::new("sk-disabled-probe".into()),
        )
        .await
        .expect("credential");
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("WorkBuddy driver");
    let transport = Arc::new(CapturingTransport::default());
    let service = ProviderCredentialTestService::new(
        Arc::new(providers),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
    );

    let result = service
        .test(snapshot, credential_id)
        .await
        .expect("credential test");

    assert_eq!(
        result.outcome,
        ProviderCredentialTestOutcome::Accepted {
            status_code: 200,
            auth_error_cleared: false,
            models: vec!["gpt-probe".into()],
        },
        "endpoint_enabled={endpoint_enabled}, credential_enabled={credential_enabled}",
    );
    let captured = transport.request.lock().expect("captured request");
    let request = captured.as_ref().expect("probe request");
    assert_eq!(request.uri.path(), "/v1/models");
    assert_eq!(request.headers[AUTHORIZATION], "Bearer sk-disabled-probe");
}

#[derive(Default)]
struct CapturingTransport {
    request: Mutex<Option<TransportRequest>>,
}

#[async_trait]
impl TransportManager for CapturingTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        *self.request.lock().expect("captured request") = Some(request);
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async {
                Ok(bytes::Bytes::from_static(
                    br#"{"data":[{"id":"gpt-probe"}]}"#,
                ))
            })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

fn endpoint_draft(enabled: bool) -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex",
        ProviderKind::Codex,
        "https://api.example.com/v1",
        ProtocolDialect::OpenAiResponses,
        enabled,
    )
    .expect("endpoint draft")
}

fn credential_draft(enabled: bool) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        Some(RequestsPerMinute::new(1).expect("valid RPM")),
        enabled,
    )
    .expect("credential draft")
}

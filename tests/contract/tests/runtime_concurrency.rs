use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProtocolOperation,
    ProviderBaseUrl, ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId,
    ProviderKind, ProxyProfileId, RequestsPerMinute, RetrySafety, UpstreamError,
    UpstreamErrorClassification, UpstreamErrorKind,
};
use any2api_provider::api::{
    CapabilitySet, CredentialHeaders, CredentialTestPlan, EndpointPlan, ProviderDriver,
    ProviderError, ProviderSecret, UpstreamResponseMeta,
};
use any2api_runtime::api::{
    ConfigPublisher, ProviderApiKeySecret, PublishedSnapshot, RoutingPermit, RuntimeRegistry,
    SelectAndReserveResult, SnapshotStore, select_and_try_reserve,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::http::{HeaderMap, HeaderValue, header::AUTHORIZATION};
use tempfile::tempdir;

#[tokio::test]
async fn published_credentials_reuse_rpm_windows_and_isolate_secret_generations() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("any2api.sqlite3");
    let storage = Arc::new(SqliteStore::connect(&database).await.expect("storage"));
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(
            configuration,
            runtime.as_ref(),
            any2api_contract_tests::build_provider_registry().as_ref(),
        )
        .expect("initial snapshot"),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        any2api_contract_tests::build_configuration_capabilities(),
    )
    .expect("configuration publisher");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let driver = HeaderEchoDriver::default();

    let endpoint = publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex Primary",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                None,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("endpoint publish");
    let created = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(2),
            ProviderApiKeySecret::new("sk-runtime-initial".to_owned()),
        )
        .await
        .expect("credential publish");
    let initial_binding = created
        .credential_runtime(credential_id.into())
        .expect("initial runtime")
        .clone();
    let old_permit = reserve(&initial_binding);
    assert_bearer(&old_permit, &driver, "sk-runtime-initial");

    let lowered = publisher
        .update_provider_credential(created.revision(), credential_id, 1, credential_draft(1))
        .await
        .expect("RPM update");
    let lowered_binding = lowered
        .credential_runtime(credential_id.into())
        .expect("lowered runtime");
    assert_eq!(lowered_binding.in_flight(), 1);
    assert_eq!(
        lowered_binding.rate_snapshot().requests_per_minute(),
        Some(1)
    );
    assert_eq!(lowered_binding.rate_snapshot().requests_in_window(), 1);
    assert_rate_limited(lowered_binding);
    assert_eq!(lowered_binding.generation().routing_generation(), 1);

    let rotated = publisher
        .rotate_provider_credential_secret(
            lowered.revision(),
            credential_id,
            2,
            1,
            ProviderApiKeySecret::new("sk-runtime-rotated".to_owned()),
        )
        .await
        .expect("secret rotation");
    let rotated_binding = rotated
        .credential_runtime(credential_id.into())
        .expect("rotated runtime")
        .clone();
    assert_eq!(old_permit.generation().routing_generation(), 1);
    assert_eq!(rotated_binding.generation().routing_generation(), 2);
    assert_eq!(rotated_binding.generation().authentication_version(), 2);
    assert_eq!(rotated_binding.in_flight(), 1);
    assert_eq!(rotated_binding.rate_snapshot().requests_in_window(), 1);
    assert_bearer(&old_permit, &driver, "sk-runtime-initial");

    drop(old_permit);
    assert_eq!(rotated_binding.in_flight(), 0);
    assert_rate_limited(&rotated_binding);
    let raised = publisher
        .update_provider_credential(rotated.revision(), credential_id, 3, credential_draft(3))
        .await
        .expect("raised RPM");
    let raised_binding = raised
        .credential_runtime(credential_id.into())
        .expect("raised runtime");
    let new_permit = reserve(raised_binding);
    assert_eq!(new_permit.generation().routing_generation(), 2);
    assert_bearer(&new_permit, &driver, "sk-runtime-rotated");

    let restarted_storage = SqliteStore::connect(&database)
        .await
        .expect("restarted storage");
    let restarted_configuration = restarted_storage
        .load_configuration()
        .await
        .expect("restarted configuration");
    let restarted_runtime = RuntimeRegistry::new();
    let providers = any2api_contract_tests::build_provider_registry();
    let restarted_snapshot = PublishedSnapshot::new(
        restarted_configuration,
        &restarted_runtime,
        providers.as_ref(),
    )
    .expect("restarted snapshot");
    let restarted_binding = restarted_snapshot
        .credential_runtime(credential_id.into())
        .expect("restarted credential runtime");
    let restarted_permit = reserve(restarted_binding);
    assert_bearer(&restarted_permit, &driver, "sk-runtime-rotated");
    assert_eq!(restarted_runtime.scheduler_epoch(), 0);
    drop(restarted_permit);

    let deleted = publisher
        .delete_provider_credential(raised.revision(), credential_id, 4)
        .await
        .expect("credential delete");
    assert!(deleted.credential_runtime(credential_id.into()).is_none());
    assert_eq!(runtime.active_credential_count(), 0);
    let post_delete_permit = reserve(raised_binding);
    assert_eq!(post_delete_permit.generation().routing_generation(), 2);
    assert_bearer(&post_delete_permit, &driver, "sk-runtime-rotated");
    drop(new_permit);
    drop(post_delete_permit);
    assert_eq!(rotated_binding.in_flight(), 0);
}

fn reserve(binding: &any2api_runtime::api::CredentialRuntimeBinding) -> RoutingPermit {
    match select_and_try_reserve(std::slice::from_ref(binding), 0) {
        SelectAndReserveResult::Reserved(permit) => permit,
        result => panic!("expected RPM reservation, got {result:?}"),
    }
}

fn assert_rate_limited(binding: &any2api_runtime::api::CredentialRuntimeBinding) {
    assert!(matches!(
        select_and_try_reserve(std::slice::from_ref(binding), 0),
        SelectAndReserveResult::RateLimited { .. }
    ));
}

fn assert_bearer(permit: &RoutingPermit, driver: &HeaderEchoDriver, api_key: &str) {
    let base_url = ProviderBaseUrl::parse("https://api.example.com/v1").expect("base URL");
    let headers = permit
        .credential_headers(driver, &base_url, &HeaderMap::new())
        .expect("credential headers");
    assert_eq!(
        headers
            .headers
            .get(AUTHORIZATION)
            .expect("authorization header"),
        &HeaderValue::from_str(&format!("Bearer {api_key}")).expect("header value")
    );
}

#[derive(Default)]
struct HeaderEchoDriver {
    capabilities: CapabilitySet,
}

impl ProviderDriver for HeaderEchoDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Codex
    }

    fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    fn validate_credential(&self, _secret: &ProviderSecret) -> Result<(), ProviderError> {
        Ok(())
    }

    fn endpoint_plan(
        &self,
        base_url: &ProviderBaseUrl,
        _operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        Ok(EndpointPlan {
            url: base_url.as_str().parse().expect("validated endpoint URL"),
        })
    }

    fn credential_headers(
        &self,
        _base_url: &ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        let mut headers = CredentialHeaders::default();
        headers.headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", secret.expose()))
                .map_err(|error| ProviderError::InvalidCredential(error.to_string()))?,
        );
        Ok(headers)
    }

    fn credential_test_plan(
        &self,
        base_url: &ProviderBaseUrl,
    ) -> Result<CredentialTestPlan, ProviderError> {
        Ok(CredentialTestPlan {
            url: base_url.as_str().parse().expect("validated endpoint URL"),
            headers: HeaderMap::new(),
        })
    }

    fn parse_model_catalog(&self, _bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        Ok(Vec::new())
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        _meta: &UpstreamResponseMeta,
        _bounded_body: &[u8],
    ) -> UpstreamError {
        UpstreamError::new(
            UpstreamErrorClassification::new(
                UpstreamErrorKind::Unknown,
                RetrySafety::Ambiguous,
                None,
            ),
            None,
        )
    }
}

fn credential_draft(requests_per_minute: u32) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        Some(RequestsPerMinute::new(requests_per_minute).expect("valid RPM")),
        true,
    )
    .expect("credential draft")
}

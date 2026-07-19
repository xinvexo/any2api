use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ErrorClass, MaxConcurrency, ProtocolDialect,
    ProtocolOperation, ProviderBaseUrl, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId, ProviderKind, ProxyProfileId,
};
use any2api_provider::api::{
    CapabilitySet, CredentialHeaders, EndpointPlan, ProviderDriver, ProviderError, ProviderSecret,
    UpstreamResponseMeta,
};
use any2api_runtime::api::{
    ConcurrencyPermit, ConfigPublisher, ProviderApiKeySecret, PublishedSnapshot, RuntimeRegistry,
    SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::http::{HeaderValue, header::AUTHORIZATION};
use tempfile::tempdir;

#[tokio::test]
async fn published_credentials_reuse_capacity_and_isolate_secret_generations() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("any2api.sqlite3");
    let storage = Arc::new(SqliteStore::connect(&database).await.expect("storage"));
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    );
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
                false,
                false,
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
        .credential_runtime(credential_id)
        .expect("initial runtime")
        .clone();
    let old_permit = initial_binding.try_acquire().expect("initial permit");
    assert_bearer(&old_permit, &driver, "sk-runtime-initial");

    let lowered = publisher
        .update_provider_credential(created.revision(), credential_id, 1, credential_draft(1))
        .await
        .expect("capacity update");
    let lowered_binding = lowered
        .credential_runtime(credential_id)
        .expect("lowered runtime");
    assert_eq!(lowered_binding.capacity().in_flight(), 1);
    assert_eq!(lowered_binding.capacity().max_concurrency(), 1);
    assert!(lowered_binding.try_acquire().is_none());
    assert_eq!(lowered_binding.generation().credential_generation(), 1);

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
        .credential_runtime(credential_id)
        .expect("rotated runtime")
        .clone();
    assert_eq!(old_permit.generation().credential_generation(), 1);
    assert_eq!(rotated_binding.generation().credential_generation(), 2);
    assert_eq!(rotated_binding.generation().secret_version(), 2);
    assert_eq!(rotated_binding.capacity().in_flight(), 1);
    assert_bearer(&old_permit, &driver, "sk-runtime-initial");

    drop(old_permit);
    let new_permit = rotated_binding.try_acquire().expect("rotated permit");
    assert_eq!(new_permit.generation().credential_generation(), 2);
    assert_bearer(&new_permit, &driver, "sk-runtime-rotated");

    let restarted_storage = SqliteStore::connect(&database)
        .await
        .expect("restarted storage");
    let restarted_configuration = restarted_storage
        .load_configuration()
        .await
        .expect("restarted configuration");
    let restarted_runtime = RuntimeRegistry::new();
    let restarted_snapshot = PublishedSnapshot::new(restarted_configuration, &restarted_runtime);
    let restarted_permit = restarted_snapshot
        .credential_runtime(credential_id)
        .expect("restarted credential runtime")
        .try_acquire()
        .expect("restarted permit");
    assert_bearer(&restarted_permit, &driver, "sk-runtime-rotated");
    assert_eq!(restarted_runtime.scheduler_epoch(), 0);
    drop(restarted_permit);

    let deleted = publisher
        .delete_provider_credential(rotated.revision(), credential_id, 3)
        .await
        .expect("credential delete");
    assert!(deleted.credential_runtime(credential_id).is_none());
    assert_eq!(runtime.active_credential_count(), 0);
    assert!(rotated_binding.is_retired());
    drop(new_permit);
}

fn assert_bearer(permit: &ConcurrencyPermit, driver: &HeaderEchoDriver, api_key: &str) {
    let headers = permit
        .provider_credential_headers(driver)
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

    fn classify_error(&self, _meta: &UpstreamResponseMeta, _bounded_body: &[u8]) -> ErrorClass {
        ErrorClass::Upstream
    }
}

fn credential_draft(max_concurrency: u32) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        MaxConcurrency::new(max_concurrency).expect("max concurrency"),
        true,
    )
    .expect("credential draft")
}

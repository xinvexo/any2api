use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId,
};
use any2api_runtime::api::{
    ConfigPublisher, ProviderApiKeySecret, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

#[tokio::test]
async fn published_credentials_reuse_capacity_and_isolate_secret_generations() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("storage"),
    );
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

    drop(old_permit);
    let new_permit = rotated_binding.try_acquire().expect("rotated permit");
    assert_eq!(new_permit.generation().credential_generation(), 2);

    let deleted = publisher
        .delete_provider_credential(rotated.revision(), credential_id, 3)
        .await
        .expect("credential delete");
    assert!(deleted.credential_runtime(credential_id).is_none());
    assert_eq!(runtime.active_credential_count(), 0);
    assert!(rotated_binding.is_retired());
    drop(new_permit);
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

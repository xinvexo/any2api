use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId, PublicModelName, TransportMode,
};
use any2api_provider::{CodexDriver, api::ProviderRegistry};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

use crate::{
    provider_api_key_secret::ProviderApiKeySecret,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    registry::RuntimeRegistry,
    route_candidates::build_route_candidates,
};

#[tokio::test]
async fn credentials_on_same_endpoint_only_serve_their_selected_models() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new(initial.settings().scheduler()));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial,
        runtime.as_ref(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    );
    let endpoint_id = ProviderEndpointId::new();
    let first_id = CredentialId::new();
    let second_id = CredentialId::new();

    let endpoint = publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, endpoint_draft())
        .await
        .expect("endpoint");
    let first = publisher
        .create_provider_credential(
            endpoint.revision(),
            first_id,
            endpoint_id,
            credential_draft("First"),
            ProviderApiKeySecret::new("sk-first-model-key".to_owned()),
        )
        .await
        .expect("first credential");
    let second = publisher
        .create_provider_credential(
            first.revision(),
            second_id,
            endpoint_id,
            credential_draft("Second"),
            ProviderApiKeySecret::new("sk-second-model-key".to_owned()),
        )
        .await
        .expect("second credential");
    let first_models = publisher
        .set_provider_credential_models(
            second.revision(),
            first_id,
            1,
            vec!["model-first".to_owned()],
        )
        .await
        .expect("first models");
    let snapshot = publisher
        .set_provider_credential_models(
            first_models.revision(),
            second_id,
            1,
            vec!["model-second".to_owned()],
        )
        .await
        .expect("second models");

    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");

    assert_eq!(
        candidates_for(&snapshot, &providers, "model-first"),
        vec![first_id]
    );
    assert_eq!(
        candidates_for(&snapshot, &providers, "model-second"),
        vec![second_id]
    );
}

fn candidates_for(
    snapshot: &PublishedSnapshot,
    providers: &ProviderRegistry,
    model: &str,
) -> Vec<CredentialId> {
    let model = PublicModelName::new(model).expect("public model");
    let route = snapshot
        .model_routes()
        .resolve(ProtocolDialect::OpenAiResponses, &model)
        .expect("derived route");
    let candidates = build_route_candidates(snapshot, route, providers, TransportMode::Json);
    candidates
        .values()
        .flatten()
        .map(|candidate| candidate.credential_id)
        .collect()
}

fn endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com/v1",
        ProtocolDialect::OpenAiResponses,
        true,
    )
    .expect("endpoint draft")
}

fn credential_draft(label: &str) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        MaxConcurrency::new(4).expect("max concurrency"),
        true,
    )
    .expect("credential draft")
}

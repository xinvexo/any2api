use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, FallbackTier, ModelRouteDraft, ModelRouteId, ProtocolDialect,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, RouteTargetDraft, RouteTargetId,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::{TempDir, tempdir};

use crate::{
    config_publish_error::ConfigPublishError,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn model_route_publish_is_atomic_and_preserves_target_identity() {
    let context = TestContext::new().await;
    let endpoint_id = ProviderEndpointId::new();
    let endpoint = context
        .publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex",
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
        .expect("endpoint");
    let route_id = ModelRouteId::new();
    let target_id = RouteTargetId::new();
    let published = context
        .publisher
        .create_model_route(
            endpoint.revision(),
            route_id,
            route_draft(target_id, endpoint_id, "gpt-5.1", 0),
        )
        .await
        .expect("route");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(published.revision(), stored.revision());
    assert_eq!(published.model_routes(), stored.model_routes());
    assert_eq!(
        published
            .model_routes()
            .get(route_id)
            .expect("route")
            .targets()[0]
            .id(),
        target_id
    );
    assert_eq!(context.runtime.scheduler_epoch(), 2);

    let error = context
        .publisher
        .update_model_route(
            published.revision(),
            route_id,
            1,
            route_draft(target_id, endpoint_id, "different-upstream", 0),
        )
        .await
        .expect_err("target identity change");
    assert!(matches!(
        error,
        ConfigPublishError::RouteTargetIdentityConflict
    ));
    assert_eq!(context.snapshots.load().revision(), published.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 2);
}

struct TestContext {
    _directory: TempDir,
    repository: Arc<SqliteStore>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: ConfigPublisher,
}

impl TestContext {
    async fn new() -> Self {
        let directory = tempdir().expect("temporary directory");
        let repository = Arc::new(
            SqliteStore::connect(&directory.path().join("config.sqlite3"))
                .await
                .expect("repository"),
        );
        let initial = repository
            .load_configuration()
            .await
            .expect("initial configuration");
        let runtime = Arc::new(RuntimeRegistry::new(initial.settings().scheduler()));
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            initial,
            runtime.as_ref(),
        )));
        let publisher = ConfigPublisher::new(
            Arc::clone(&repository),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
        );
        Self {
            _directory: directory,
            repository,
            snapshots,
            runtime,
            publisher,
        }
    }
}

fn route_draft(
    target_id: RouteTargetId,
    endpoint_id: ProviderEndpointId,
    upstream_model: &str,
    tier: u16,
) -> ModelRouteDraft {
    ModelRouteDraft::new(
        "codex-public",
        ProtocolDialect::OpenAiResponses,
        None,
        true,
        vec![
            RouteTargetDraft::new(
                target_id,
                endpoint_id,
                upstream_model,
                FallbackTier::new(tier),
                true,
            )
            .expect("target draft"),
        ],
    )
    .expect("route draft")
}

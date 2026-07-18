use any2api_domain::{
    ConfigRevision, FallbackTier, ModelRouteDraft, ModelRouteId, ProtocolDialect,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, RouteTargetDraft, RouteTargetId,
};
use tempfile::tempdir;

use crate::{
    configuration_repository::ConfigurationRepository, error::StorageError,
    model_route_repository::ModelRouteRepository, sqlite::SqliteStore,
};

#[tokio::test]
async fn model_route_aggregate_persists_and_preserves_target_identity() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let endpoint = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            endpoint_draft(ProviderKind::Codex),
        )
        .await
        .expect("endpoint");
    let route_id = ModelRouteId::new();
    let target_id = RouteTargetId::new();
    let created = store
        .create_model_route(
            endpoint.revision(),
            route_id,
            route_draft(
                "codex",
                ProtocolDialect::OpenAiResponses,
                endpoint_id,
                target_id,
                0,
            ),
        )
        .await
        .expect("route");
    assert_eq!(created.revision().get(), 3);
    assert_eq!(
        created
            .model_routes()
            .get(route_id)
            .expect("route")
            .config_version(),
        1
    );

    let updated = store
        .update_model_route(
            created.revision(),
            route_id,
            1,
            route_draft(
                "codex-local",
                ProtocolDialect::OpenAiResponses,
                endpoint_id,
                target_id,
                2,
            ),
        )
        .await
        .expect("updated route");
    let route = updated.model_routes().get(route_id).expect("updated route");
    assert_eq!(route.config_version(), 2);
    assert_eq!(route.targets()[0].id(), target_id);
    assert_eq!(route.targets()[0].fallback_tier().get(), 2);

    let reopened = SqliteStore::connect(&database)
        .await
        .expect("reopened store");
    let loaded = reopened.load_configuration().await.expect("configuration");
    assert_eq!(loaded.model_routes(), updated.model_routes());
}

#[tokio::test]
async fn route_conflicts_do_not_advance_revision_and_protect_endpoints() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let codex_id = ProviderEndpointId::new();
    let claude_id = ProviderEndpointId::new();
    let codex = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            codex_id,
            endpoint_draft(ProviderKind::Codex),
        )
        .await
        .expect("codex endpoint");
    let endpoints = store
        .create_provider_endpoint(
            codex.revision(),
            claude_id,
            endpoint_draft(ProviderKind::Claude),
        )
        .await
        .expect("claude endpoint");

    let error = store
        .create_model_route(
            endpoints.revision(),
            ModelRouteId::new(),
            route_draft(
                "cross",
                ProtocolDialect::OpenAiResponses,
                claude_id,
                RouteTargetId::new(),
                0,
            ),
        )
        .await
        .expect_err("cross protocol route");
    assert!(matches!(error, StorageError::ModelRouteValidation(_)));
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("configuration")
            .revision(),
        endpoints.revision()
    );

    let route = store
        .create_model_route(
            endpoints.revision(),
            ModelRouteId::new(),
            route_draft(
                "shared",
                ProtocolDialect::OpenAiResponses,
                codex_id,
                RouteTargetId::new(),
                0,
            ),
        )
        .await
        .expect("route");
    let duplicate = store
        .create_model_route(
            route.revision(),
            ModelRouteId::new(),
            route_draft(
                "shared",
                ProtocolDialect::OpenAiResponses,
                codex_id,
                RouteTargetId::new(),
                1,
            ),
        )
        .await
        .expect_err("duplicate route");
    assert!(matches!(duplicate, StorageError::ModelRouteNameConflict));
    assert!(matches!(
        store
            .delete_provider_endpoint(route.revision(), codex_id)
            .await,
        Err(StorageError::ProviderEndpointInUse)
    ));
}

fn endpoint_draft(kind: ProviderKind) -> ProviderEndpointDraft {
    let dialect = match kind {
        ProviderKind::Codex => ProtocolDialect::OpenAiResponses,
        ProviderKind::Claude => ProtocolDialect::AnthropicMessages,
    };
    ProviderEndpointDraft::new(
        format!("{kind:?}"),
        kind,
        "https://api.example.com",
        dialect,
        false,
        false,
        true,
    )
    .expect("endpoint draft")
}

fn route_draft(
    public_model: &str,
    dialect: ProtocolDialect,
    endpoint_id: ProviderEndpointId,
    target_id: RouteTargetId,
    tier: u16,
) -> ModelRouteDraft {
    ModelRouteDraft::new(
        public_model,
        dialect,
        Some(true),
        true,
        vec![
            RouteTargetDraft::new(
                target_id,
                endpoint_id,
                "upstream-model",
                FallbackTier::new(tier),
                true,
            )
            .expect("target draft"),
        ],
    )
    .expect("route draft")
}

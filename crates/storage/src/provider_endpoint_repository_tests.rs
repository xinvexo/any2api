use any2api_domain::{
    ConfigRevision, ProtocolDialect, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationRepository, SqliteStore},
    error::StorageError,
};

#[tokio::test]
async fn new_database_starts_without_provider_endpoints() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");

    let configuration = store.load_configuration().await.expect("configuration");

    assert!(configuration.provider_endpoints().endpoints().is_empty());
}

#[tokio::test]
async fn provider_endpoint_crud_uses_the_global_configuration_revision() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let id = ProviderEndpointId::new();

    let created = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            id,
            codex_draft("https://api.example.com/v1/"),
        )
        .await
        .expect("create endpoint");
    let no_op = store
        .update_provider_endpoint(
            created.revision(),
            id,
            1,
            codex_draft("https://api.example.com/v1"),
        )
        .await
        .expect("no-op update");
    let updated = store
        .update_provider_endpoint(
            no_op.revision(),
            id,
            1,
            codex_draft("https://edge.example.com/openai"),
        )
        .await
        .expect("update endpoint");
    let endpoint = updated
        .provider_endpoints()
        .get(id)
        .expect("stored endpoint");

    assert_eq!(created.revision().get(), 2);
    assert_eq!(no_op.revision(), created.revision());
    assert_eq!(updated.revision().get(), 3);
    assert_eq!(endpoint.config_version(), 2);
    assert_eq!(
        endpoint.base_url().as_str(),
        "https://edge.example.com/openai"
    );

    let stale = store
        .update_provider_endpoint(
            updated.revision(),
            id,
            1,
            codex_draft("https://stale.example.com"),
        )
        .await
        .expect_err("stale endpoint version must fail");
    assert!(matches!(
        stale,
        StorageError::ProviderEndpointVersionConflict {
            expected: 1,
            actual: 2
        }
    ));

    let deleted = store
        .delete_provider_endpoint(updated.revision(), id)
        .await
        .expect("delete endpoint");
    assert_eq!(deleted.revision().get(), 4);
    assert!(deleted.provider_endpoints().endpoints().is_empty());
}

#[tokio::test]
async fn duplicate_endpoint_names_are_rejected_before_commit() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let first = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            ProviderEndpointId::new(),
            codex_draft("https://api.example.com"),
        )
        .await
        .expect("first endpoint");

    let error = store
        .create_provider_endpoint(
            first.revision(),
            ProviderEndpointId::new(),
            codex_draft("https://edge.example.com"),
        )
        .await
        .expect_err("duplicate name must fail");

    assert!(matches!(error, StorageError::ProviderEndpointNameConflict));
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("configuration")
            .revision(),
        first.revision()
    );
}

#[tokio::test]
async fn unsafe_database_rows_fail_configuration_loading() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          allow_insecure_http, allow_private_network, enabled, config_version) \
         VALUES (?, 'Unsafe', 'unsafe', 'codex', 'http://127.0.0.1:8080', \
                 'openai_responses', 0, 0, 1, 1)",
    )
    .bind(ProviderEndpointId::new().to_string())
    .execute(store.pool())
    .await
    .expect("insert unsafe row");

    let error = store
        .load_configuration()
        .await
        .expect_err("unsafe stored URL must fail startup loading");
    assert!(matches!(error, StorageError::CorruptConfiguration));
}

fn codex_draft(base_url: &str) -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        base_url,
        ProtocolDialect::OpenAiResponses,
        false,
        false,
        true,
    )
    .expect("endpoint draft")
}

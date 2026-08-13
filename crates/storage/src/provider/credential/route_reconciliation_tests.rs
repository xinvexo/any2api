use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyProfileId, RequestId,
    RouteTargetId,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationMutation, SecretBytes, SqliteStore},
    configuration::{StoredConfiguration, commit_configuration},
};

#[tokio::test]
async fn route_reconciliation_preserves_only_still_valid_target_history() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("route-reconciliation.sqlite3"))
        .await
        .expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let created = create_credential(&store, endpoint_id, credential_id).await;
    let selected = set_models(&store, &created, credential_id, 1, &["gpt-a"]).await;
    let retained_target = target_id(&selected, "gpt-a");
    let retained_rowid = target_rowid(&store, retained_target).await;
    let retained_request = RequestId::new();
    insert_attempt(&store, retained_request, retained_target).await;

    let expanded = set_models(&store, &selected, credential_id, 2, &["gpt-a", "gpt-b"]).await;
    assert_eq!(target_id(&expanded, "gpt-a"), retained_target);
    assert_eq!(target_rowid(&store, retained_target).await, retained_rowid);
    assert_eq!(
        attempt_target(&store, retained_request).await,
        Some(retained_target)
    );

    let removed_target = target_id(&expanded, "gpt-b");
    let removed_request = RequestId::new();
    insert_attempt(&store, removed_request, removed_target).await;
    let reduced = set_models(&store, &expanded, credential_id, 3, &["gpt-a"]).await;

    assert_eq!(target_id(&reduced, "gpt-a"), retained_target);
    assert_eq!(
        attempt_target(&store, retained_request).await,
        Some(retained_target)
    );
    assert_eq!(attempt_target(&store, removed_request).await, None);
}

async fn create_credential(
    store: &SqliteStore,
    endpoint_id: ProviderEndpointId,
    credential_id: CredentialId,
) -> StoredConfiguration {
    let endpoint = commit_configuration(
        store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: ProviderEndpointDraft::new(
                "Codex",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                None,
                true,
            )
            .expect("endpoint draft"),
        },
    )
    .await
    .expect("endpoint");
    commit_configuration(
        store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: ProviderCredentialDraft::new(
                "Primary",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                None,
                true,
            )
            .expect("credential draft"),
            api_key: SecretBytes::from(b"sk-route-reconciliation".to_vec()),
        },
    )
    .await
    .expect("credential")
}

async fn set_models(
    store: &SqliteStore,
    current: &StoredConfiguration,
    credential_id: CredentialId,
    expected_config_version: u64,
    models: &[&str],
) -> StoredConfiguration {
    commit_configuration(
        store,
        current.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version,
            models: models.iter().map(|model| (*model).to_owned()).collect(),
        },
    )
    .await
    .expect("set models")
}

fn target_id(configuration: &StoredConfiguration, model: &str) -> RouteTargetId {
    configuration
        .model_routes()
        .routes()
        .iter()
        .find(|route| route.public_model().as_str() == model)
        .expect("model route")
        .targets()[0]
        .id()
}

async fn target_rowid(store: &SqliteStore, target_id: RouteTargetId) -> i64 {
    sqlx::query_scalar("SELECT rowid FROM route_targets WHERE id = ?")
        .bind(target_id.to_string())
        .fetch_one(store.pool())
        .await
        .expect("target rowid")
}

async fn insert_attempt(store: &SqliteStore, request_id: RequestId, target_id: RouteTargetId) {
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, status_code, \
          attempt_count, latency_ms, is_stream, client_ip) \
         VALUES (?, 1, 1, 'openai_responses', 'responses', 200, 1, 1, 0, '127.0.0.1')",
    )
    .bind(request_id.to_string())
    .execute(store.pool())
    .await
    .expect("request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, route_target_id, started_at_ms, duration_ms, outcome) \
         VALUES (?, 1, ?, 1, 1, 'success')",
    )
    .bind(request_id.to_string())
    .bind(target_id.to_string())
    .execute(store.pool())
    .await
    .expect("request attempt");
}

async fn attempt_target(store: &SqliteStore, request_id: RequestId) -> Option<RouteTargetId> {
    let stored = sqlx::query_scalar::<_, Option<String>>(
        "SELECT route_target_id FROM request_attempts WHERE request_id = ? AND attempt_no = 1",
    )
    .bind(request_id.to_string())
    .fetch_one(store.pool())
    .await
    .expect("attempt target");
    stored.map(|value| value.parse().expect("route target id"))
}

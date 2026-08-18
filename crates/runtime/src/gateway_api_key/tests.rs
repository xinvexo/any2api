use std::sync::Arc;

use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigPublisher, PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn gateway_auth_material_is_isolated_by_published_snapshot() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("store"),
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
        storage,
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let id = GatewayApiKeyId::new();

    let created = publisher
        .create_gateway_api_key(
            ConfigRevision::INITIAL,
            id,
            GatewayApiKeyDraft::new("CLI", true).expect("draft"),
        )
        .await
        .expect("create");
    let first_token = created
        .gateway_api_keys()
        .get(id)
        .expect("created key")
        .token()
        .to_owned();
    let first_snapshot = created;
    assert_eq!(
        first_snapshot
            .authenticate_gateway_api_key(&first_token)
            .map(|proof| proof.id()),
        Some(id)
    );
    let legacy_token = format!(
        "a2k_v1_{}",
        first_token
            .strip_prefix("sk-")
            .expect("current token prefix")
    );
    assert_eq!(
        first_snapshot.authenticate_gateway_api_key(&legacy_token),
        None
    );

    let first_key = first_snapshot.gateway_api_keys().get(id).expect("key");
    let rotated = publisher
        .rotate_gateway_api_key(
            first_snapshot.revision(),
            id,
            first_key.config_version(),
            first_key.token_version(),
        )
        .await
        .expect("rotate");
    let second_token = rotated
        .gateway_api_keys()
        .get(id)
        .expect("rotated key")
        .token()
        .to_owned();
    let second_snapshot = rotated;
    assert_eq!(
        first_snapshot
            .authenticate_gateway_api_key(&first_token)
            .map(|proof| proof.id()),
        Some(id)
    );
    assert_eq!(
        second_snapshot.authenticate_gateway_api_key(&first_token),
        None
    );
    assert_eq!(
        second_snapshot
            .authenticate_gateway_api_key(&second_token)
            .map(|proof| proof.id()),
        Some(id)
    );

    let second_key = second_snapshot.gateway_api_keys().get(id).expect("key");
    publisher
        .delete_gateway_api_key(second_snapshot.revision(), id, second_key.config_version())
        .await
        .expect("delete");
    let deleted_snapshot = snapshots.load();
    assert_eq!(
        second_snapshot
            .authenticate_gateway_api_key(&second_token)
            .map(|proof| proof.id()),
        Some(id)
    );
    assert!(deleted_snapshot.gateway_api_keys().get(id).is_none());
    assert_eq!(
        deleted_snapshot.authenticate_gateway_api_key(&second_token),
        None
    );
    assert_eq!(runtime.scheduler_epoch(), 3);
}

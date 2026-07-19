use std::sync::Arc;

use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

use crate::{
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
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
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
    )));
    let publisher = ConfigPublisher::new(storage, Arc::clone(&snapshots), Arc::clone(&runtime));
    let id = GatewayApiKeyId::new();

    let created = publisher
        .create_gateway_api_key(
            ConfigRevision::INITIAL,
            id,
            GatewayApiKeyDraft::new("CLI", true).expect("draft"),
        )
        .await
        .expect("create");
    let first_token = created.token().as_str().to_owned();
    let first_snapshot = snapshots.load();
    assert_eq!(
        first_snapshot.authenticate_gateway_api_key(&first_token),
        Some(id)
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
    let second_token = rotated.token().as_str().to_owned();
    let second_snapshot = snapshots.load();
    assert_eq!(
        first_snapshot.authenticate_gateway_api_key(&first_token),
        Some(id)
    );
    assert_eq!(
        second_snapshot.authenticate_gateway_api_key(&first_token),
        None
    );
    assert_eq!(
        second_snapshot.authenticate_gateway_api_key(&second_token),
        Some(id)
    );

    let second_key = second_snapshot.gateway_api_keys().get(id).expect("key");
    publisher
        .revoke_gateway_api_key(second_snapshot.revision(), id, second_key.config_version())
        .await
        .expect("revoke");
    let revoked_snapshot = snapshots.load();
    assert_eq!(
        second_snapshot.authenticate_gateway_api_key(&second_token),
        Some(id)
    );
    assert_eq!(
        revoked_snapshot.authenticate_gateway_api_key(&second_token),
        None
    );
    assert_eq!(runtime.scheduler_epoch(), 3);
}

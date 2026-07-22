use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::ExposeSecret;
use tempfile::tempdir;

use crate::gateway_api_key_usage_repository::{
    GatewayApiKeyLastUsedUpdate, GatewayApiKeyUsageRepository,
};
use crate::{
    configuration_repository::ConfigurationRepository, error::StorageError,
    gateway_api_key_repository::GatewayApiKeyRepository, sqlite::SqliteStore, vault::SecretBytes,
};

#[tokio::test]
async fn gateway_api_key_lifecycle_persists_only_the_hash() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("any2api.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let id = GatewayApiKeyId::new();
    let first = token(7);

    let created = store
        .create_gateway_api_key(
            ConfigRevision::INITIAL,
            id,
            GatewayApiKeyDraft::new("Desktop", true).expect("draft"),
            secret(&first),
        )
        .await
        .expect("create");
    assert_eq!(created.revision().get(), 2);
    let key = created.gateway_api_keys().get(id).expect("created key");
    assert!(
        created
            .gateway_api_key_verifier()
            .verify(first.as_bytes(), key.token_hash())
    );
    assert_ne!(key.token_prefix(), first);
    let stored: (String, i64) = sqlx::query_as(
        "SELECT token_prefix, length(token_hash) FROM gateway_api_keys WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_one(store.pool())
    .await
    .expect("stored row");
    assert_eq!(stored.0, key.token_prefix());
    assert_eq!(stored.1, 32);

    let unchanged = store
        .update_gateway_api_key(
            created.revision(),
            id,
            key.config_version(),
            GatewayApiKeyDraft::new("Desktop", true).expect("draft"),
        )
        .await
        .expect("no-op update");
    assert_eq!(unchanged.revision(), created.revision());

    let second = token(9);
    let rotated = store
        .rotate_gateway_api_key(
            created.revision(),
            id,
            key.config_version(),
            key.token_version(),
            secret(&second),
        )
        .await
        .expect("rotate");
    let rotated_key = rotated.gateway_api_keys().get(id).expect("rotated key");
    assert_eq!(rotated_key.token_version(), 2);
    assert!(
        !rotated
            .gateway_api_key_verifier()
            .verify(first.as_bytes(), rotated_key.token_hash())
    );
    assert!(
        rotated
            .gateway_api_key_verifier()
            .verify(second.as_bytes(), rotated_key.token_hash())
    );

    let revoked = store
        .revoke_gateway_api_key(rotated.revision(), id, rotated_key.config_version())
        .await
        .expect("revoke");
    let revoked_key = revoked.gateway_api_keys().get(id).expect("revoked key");
    assert!(revoked_key.is_revoked());
    assert!(!revoked_key.is_active());

    drop(store);
    let reopened = SqliteStore::connect(&database).await.expect("reopen");
    let loaded = reopened.load_configuration().await.expect("load");
    let loaded_key = loaded.gateway_api_keys().get(id).expect("loaded key");
    assert_eq!(loaded_key, revoked_key);
    assert!(
        loaded
            .gateway_api_key_verifier()
            .verify(second.as_bytes(), loaded_key.token_hash())
    );
}

#[tokio::test]
async fn gateway_api_key_conflicts_do_not_advance_revision() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
        .await
        .expect("store");
    let first_id = GatewayApiKeyId::new();
    let created = store
        .create_gateway_api_key(
            ConfigRevision::INITIAL,
            first_id,
            GatewayApiKeyDraft::new("CLI", true).expect("draft"),
            secret(&token(1)),
        )
        .await
        .expect("create");
    let duplicate = store
        .create_gateway_api_key(
            created.revision(),
            GatewayApiKeyId::new(),
            GatewayApiKeyDraft::new("cli", true).expect("draft"),
            secret(&token(2)),
        )
        .await
        .expect_err("duplicate name");
    assert!(matches!(duplicate, StorageError::GatewayApiKeyNameConflict));

    let key = created
        .gateway_api_keys()
        .get(first_id)
        .expect("created key");
    let stale = store
        .rotate_gateway_api_key(
            created.revision(),
            first_id,
            key.config_version(),
            key.token_version() + 1,
            secret(&token(3)),
        )
        .await
        .expect_err("stale token version");
    assert!(matches!(
        stale,
        StorageError::GatewayApiKeyTokenVersionConflict { .. }
    ));
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("configuration")
            .revision(),
        created.revision()
    );
}

#[tokio::test]
async fn gateway_api_key_last_used_updates_are_monotonic_and_do_not_publish_configuration() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
        .await
        .expect("store");
    let id = GatewayApiKeyId::new();
    let created = store
        .create_gateway_api_key(
            ConfigRevision::INITIAL,
            id,
            GatewayApiKeyDraft::new("Telemetry", true).expect("draft"),
            secret(&token(4)),
        )
        .await
        .expect("create");

    store
        .touch_gateway_api_key_last_used(&[GatewayApiKeyLastUsedUpdate {
            id,
            last_used_at: "2026-07-22 10:00:00".into(),
        }])
        .await
        .expect("first usage update");
    store
        .touch_gateway_api_key_last_used(&[
            GatewayApiKeyLastUsedUpdate {
                id,
                last_used_at: "2026-07-22 09:59:59".into(),
            },
            GatewayApiKeyLastUsedUpdate {
                id,
                last_used_at: "2026-07-22 10:01:00".into(),
            },
        ])
        .await
        .expect("monotonic usage updates");

    let loaded = store.load_configuration().await.expect("configuration");
    assert_eq!(loaded.revision(), created.revision());
    assert_eq!(
        loaded
            .gateway_api_keys()
            .get(id)
            .expect("gateway key")
            .last_used_at(),
        Some("2026-07-22 10:01:00")
    );
}

fn token(byte: u8) -> String {
    format!("a2k_v1_{}", URL_SAFE_NO_PAD.encode([byte; 32]))
}

fn secret(value: &str) -> SecretBytes {
    let secret: SecretBytes = value.as_bytes().to_vec().into();
    assert_eq!(secret.expose_secret(), value.as_bytes());
    secret
}

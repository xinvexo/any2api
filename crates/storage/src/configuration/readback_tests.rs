use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};
use tempfile::{TempDir, tempdir};

use crate::{
    configuration::{
        ConfigurationMutation, commit_configuration, load_configuration_from,
        readback_gateway_api_key_mutation, readback_proxy_mutation,
    },
    error::StorageError,
    secret::SecretBytes,
    sqlite::SqliteStore,
};

#[tokio::test]
async fn affected_gateway_readback_revalidates_the_changed_digest() {
    let (_directory, store, id) = store_with_gateway_key().await;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .expect("transaction");
    let current = load_configuration_from(&mut transaction)
        .await
        .expect("initial full validation");
    corrupt_gateway_digest(&mut transaction, id).await;

    let error = readback_gateway_api_key_mutation(&mut transaction, current)
        .await
        .expect_err("changed gateway key digest must be revalidated");

    assert!(matches!(error, StorageError::CorruptConfiguration));
}

#[tokio::test]
async fn proxy_readback_reuses_gateway_keys_verified_at_transaction_start() {
    let (_directory, store, id) = store_with_gateway_key().await;
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .expect("transaction");
    let current = load_configuration_from(&mut transaction)
        .await
        .expect("initial full validation");
    let expected_key = current
        .gateway_api_keys()
        .get(id)
        .expect("gateway key")
        .clone();

    // This direct write is deliberately outside the Proxy mutation contract. It makes a second
    // full load fail and therefore proves that the Proxy readback reused the already-verified
    // immutable Gateway aggregate instead of hashing it again.
    corrupt_gateway_digest(&mut transaction, id).await;
    let candidate = readback_proxy_mutation(&mut transaction, current)
        .await
        .expect("proxy impact readback");

    assert_eq!(candidate.gateway_api_keys().get(id), Some(&expected_key));
    assert!(matches!(
        load_configuration_from(&mut transaction).await,
        Err(StorageError::CorruptConfiguration)
    ));
}

async fn store_with_gateway_key() -> (TempDir, SqliteStore, GatewayApiKeyId) {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");
    let id = GatewayApiKeyId::new();
    commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateGatewayApiKey {
            id,
            draft: GatewayApiKeyDraft::new("Selective readback", true).expect("draft"),
            token: SecretBytes::from(token().into_bytes()),
        },
    )
    .await
    .expect("create gateway key");
    (directory, store, id)
}

async fn corrupt_gateway_digest(connection: &mut sqlx::SqliteConnection, id: GatewayApiKeyId) {
    sqlx::query("UPDATE gateway_api_keys SET token_hash = zeroblob(32) WHERE id = ?")
        .bind(id.to_string())
        .execute(connection)
        .await
        .expect("corrupt gateway digest");
}

fn token() -> String {
    format!(
        "{}{}",
        any2api_domain::GATEWAY_TOKEN_PREFIX,
        "r".repeat(any2api_domain::GATEWAY_TOKEN_BODY_LEN)
    )
}

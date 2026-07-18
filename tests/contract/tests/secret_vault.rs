use std::fs;

use any2api_domain::{CredentialId, CredentialKind, ProviderKind};
use any2api_storage::api::{
    SecretBytes, SecretContext, SecretVaultError, SqliteStore, StorageError,
};
use secrecy::ExposeSecret;
use tempfile::tempdir;

#[tokio::test]
async fn secret_envelope_survives_a_storage_restart() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("any2api.sqlite3");
    let master_key = directory.path().join("master-key.json");
    let context = SecretContext::provider_credential(
        CredentialId::new(),
        ProviderKind::Claude,
        CredentialKind::ApiKey,
        1,
        1,
    );
    let secret: SecretBytes = b"claude-provider-key".to_vec().into();
    let store = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect("initial storage");
    let envelope = store
        .secret_vault()
        .seal(context, &secret)
        .expect("encrypted envelope");
    drop(store);

    let reopened = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect("reopened storage");
    let plaintext = reopened
        .secret_vault()
        .open(context, &envelope)
        .expect("decrypted envelope");
    assert_eq!(plaintext.expose_secret(), b"claude-provider-key");
}

#[tokio::test]
async fn initialized_vault_never_regenerates_a_missing_key() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("any2api.sqlite3");
    let master_key = directory.path().join("master-key.json");
    drop(
        SqliteStore::connect_with_master_key(&database, &master_key)
            .await
            .expect("initial storage"),
    );
    fs::remove_file(&master_key).expect("remove master key");

    let error = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect_err("missing initialized key must fail");
    assert!(matches!(
        error,
        StorageError::SecretVault(SecretVaultError::MasterKeyMissing { .. })
    ));
    assert!(!master_key.exists());
}

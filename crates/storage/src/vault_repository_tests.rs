use std::fs;

use any2api_domain::{CredentialId, CredentialKind, ProviderKind};
use secrecy::ExposeSecret;
use tempfile::tempdir;

use crate::{
    api::{SecretBytes, SecretContext, SecretVaultError, SqliteStore, StorageError},
    sqlite::SqliteStore as PrivateSqliteStore,
};

#[tokio::test]
async fn store_initializes_and_reopens_the_same_vault() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let master_key = directory.path().join("master-key.json");
    let context = provider_context();
    let secret: SecretBytes = b"persistent-provider-key".to_vec().into();

    let store = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect("initial store");
    let envelope = store
        .secret_vault()
        .seal(context, &secret)
        .expect("envelope");
    assert!(master_key.is_file());
    assert_eq!(metadata_count(&store).await, 1);
    drop(store);

    let reopened = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect("reopened store");
    let plaintext = reopened
        .secret_vault()
        .open(context, &envelope)
        .expect("decrypted secret");
    assert_eq!(plaintext.expose_secret(), b"persistent-provider-key");
}

#[tokio::test]
async fn initialized_store_rejects_a_missing_master_key() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let master_key = directory.path().join("master-key.json");
    let store = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect("initial store");
    drop(store);
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

#[tokio::test]
async fn an_existing_invalid_master_key_is_never_overwritten() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let master_key = directory.path().join("master-key.json");
    fs::write(&master_key, b"not-a-master-key").expect("invalid master key fixture");

    let error = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect_err("invalid existing key must fail");
    assert!(matches!(
        error,
        StorageError::SecretVault(SecretVaultError::InvalidMasterKeyFormat)
    ));
    assert_eq!(
        fs::read(&master_key).expect("unchanged master key fixture"),
        b"not-a-master-key"
    );
}

#[tokio::test]
async fn initialized_store_rejects_a_different_master_key() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let master_key = directory.path().join("master-key.json");
    let other_database = directory.path().join("other.sqlite3");
    let other_key = directory.path().join("other-master-key.json");
    drop(
        SqliteStore::connect_with_master_key(&database, &master_key)
            .await
            .expect("initial store"),
    );
    drop(
        SqliteStore::connect_with_master_key(&other_database, &other_key)
            .await
            .expect("other store"),
    );
    fs::copy(&other_key, &master_key).expect("replace master key");

    let error = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect_err("wrong master key must fail");
    assert!(matches!(
        error,
        StorageError::SecretVault(SecretVaultError::KeyMismatch)
    ));
}

#[tokio::test]
async fn database_and_master_key_must_use_distinct_paths() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("config.sqlite3");

    let error = SqliteStore::connect_with_master_key(&path, &path)
        .await
        .expect_err("same path must fail");
    assert!(matches!(
        error,
        StorageError::SecretVault(SecretVaultError::MasterKeyPathConflictsWithDatabase)
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn unix_rejects_group_or_other_master_key_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let master_key = directory.path().join("master-key.json");
    drop(
        SqliteStore::connect_with_master_key(&database, &master_key)
            .await
            .expect("initial store"),
    );
    fs::set_permissions(&master_key, fs::Permissions::from_mode(0o640)).expect("relax permissions");

    let error = SqliteStore::connect_with_master_key(&database, &master_key)
        .await
        .expect_err("unsafe permissions must fail");
    assert!(matches!(
        error,
        StorageError::SecretVault(SecretVaultError::UnsafeMasterKeyPermissions { .. })
    ));
}

fn provider_context() -> SecretContext {
    SecretContext::provider_credential(
        CredentialId::new(),
        ProviderKind::Codex,
        CredentialKind::ApiKey,
    )
}

async fn metadata_count(store: &PrivateSqliteStore) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM secret_vault_metadata")
        .fetch_one(store.pool())
        .await
        .expect("metadata count")
}

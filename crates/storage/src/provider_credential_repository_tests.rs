use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyAddress,
    ProxyDraft, ProxyKind, ProxyProfileId,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationRepository, SecretBytes, SqliteStore},
    error::StorageError,
    vault::SecretVaultError,
};

#[tokio::test]
async fn credential_lifecycle_persists_versions_and_secret_metadata() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();

    let endpoint = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            codex_draft("Codex Primary", "https://api.example.com"),
        )
        .await
        .expect("create endpoint");
    let created = store
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft("Primary", ProxyProfileId::DIRECT, 4, true),
            secret("sk-first-credential"),
        )
        .await
        .expect("create credential");
    let credential = created
        .provider_credentials()
        .get(credential_id)
        .expect("credential");

    assert_eq!(created.revision().get(), 3);
    assert_eq!(credential.config_version(), 1);
    assert_eq!(credential.secret_version(), 1);
    assert_eq!(credential.credential_generation(), 1);
    assert_eq!(credential.fingerprint().tail(), Some("tial"));

    let no_op = store
        .update_provider_credential(
            created.revision(),
            credential_id,
            1,
            credential_draft("Primary", ProxyProfileId::DIRECT, 4, true),
        )
        .await
        .expect("no-op update");
    assert_eq!(no_op.revision(), created.revision());

    let disabled = store
        .update_provider_credential(
            no_op.revision(),
            credential_id,
            1,
            credential_draft("Primary", ProxyProfileId::DIRECT, 8, false),
        )
        .await
        .expect("disable credential");
    let disabled_credential = disabled
        .provider_credentials()
        .get(credential_id)
        .expect("disabled credential");
    assert_eq!(disabled_credential.config_version(), 2);
    assert_eq!(disabled_credential.credential_generation(), 1);

    let enabled = store
        .update_provider_credential(
            disabled.revision(),
            credential_id,
            2,
            credential_draft("Primary", ProxyProfileId::DIRECT, 8, true),
        )
        .await
        .expect("enable credential");
    let rotated = store
        .rotate_provider_credential_secret(
            enabled.revision(),
            credential_id,
            3,
            1,
            secret("sk-second-rotated"),
        )
        .await
        .expect("rotate credential");
    let rotated_credential = rotated
        .provider_credentials()
        .get(credential_id)
        .expect("rotated credential");
    let fingerprint = rotated_credential.fingerprint().clone();
    assert_eq!(rotated_credential.config_version(), 4);
    assert_eq!(rotated_credential.secret_version(), 2);
    assert_eq!(rotated_credential.credential_generation(), 3);
    assert_eq!(rotated_credential.fingerprint().tail(), Some("ated"));

    drop(store);
    let reopened = SqliteStore::connect(&database).await.expect("reopen store");
    let restored = reopened
        .load_configuration()
        .await
        .expect("restored configuration");
    let restored_credential = restored
        .provider_credentials()
        .get(credential_id)
        .expect("restored credential");
    assert_eq!(restored.revision(), rotated.revision());
    assert_eq!(restored_credential.fingerprint(), &fingerprint);
    assert_eq!(restored_credential.secret_version(), 2);
}

#[tokio::test]
async fn credential_references_protect_proxy_and_endpoint() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let proxy_id = ProxyProfileId::new();
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();

    let proxy = store
        .create_proxy(ConfigRevision::INITIAL, proxy_id, proxy_draft())
        .await
        .expect("create proxy");
    let endpoint = store
        .create_provider_endpoint(
            proxy.revision(),
            endpoint_id,
            codex_draft("Codex Primary", "https://api.example.com"),
        )
        .await
        .expect("create endpoint");
    let created = store
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft("Primary", proxy_id, 2, true),
            secret("sk-reference-test"),
        )
        .await
        .expect("create credential");

    assert!(matches!(
        store
            .delete_proxy(created.revision(), proxy_id)
            .await
            .expect_err("referenced proxy must be protected"),
        StorageError::ProxyReferenced
    ));
    assert!(matches!(
        store
            .delete_provider_endpoint(created.revision(), endpoint_id)
            .await
            .expect_err("referenced endpoint must be protected"),
        StorageError::ProviderEndpointInUse
    ));
    assert!(matches!(
        store
            .update_provider_endpoint(
                created.revision(),
                endpoint_id,
                1,
                claude_draft("Codex Primary", "https://api.anthropic.com"),
            )
            .await
            .expect_err("provider identity must stay stable"),
        StorageError::ProviderEndpointIdentityInUse
    ));

    let moved = store
        .update_provider_endpoint(
            created.revision(),
            endpoint_id,
            1,
            codex_draft("Codex Primary", "https://edge.example.com"),
        )
        .await
        .expect("change base URL");
    assert_eq!(
        moved
            .provider_credentials()
            .get(credential_id)
            .expect("credential")
            .credential_generation(),
        2
    );
}

#[tokio::test]
async fn credential_conflicts_do_not_advance_revision() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let first_id = CredentialId::new();

    let endpoint = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            codex_draft("Codex Primary", "https://api.example.com"),
        )
        .await
        .expect("create endpoint");
    let created = store
        .create_provider_credential(
            endpoint.revision(),
            first_id,
            endpoint_id,
            credential_draft("Primary", ProxyProfileId::DIRECT, 1, true),
            secret("sk-conflict-first"),
        )
        .await
        .expect("create credential");

    assert!(matches!(
        store
            .create_provider_credential(
                created.revision(),
                CredentialId::new(),
                endpoint_id,
                credential_draft("primary", ProxyProfileId::DIRECT, 1, true),
                secret("sk-conflict-second"),
            )
            .await
            .expect_err("duplicate label must fail"),
        StorageError::ProviderCredentialLabelConflict
    ));
    assert!(matches!(
        store
            .rotate_provider_credential_secret(
                created.revision(),
                first_id,
                1,
                2,
                secret("sk-stale-secret"),
            )
            .await
            .expect_err("stale secret version must fail"),
        StorageError::ProviderCredentialSecretVersionConflict {
            expected: 2,
            actual: 1
        }
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
async fn corrupted_credential_ciphertext_fails_configuration_loading() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = store
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            codex_draft("Codex Primary", "https://api.example.com"),
        )
        .await
        .expect("create endpoint");
    store
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft("Primary", ProxyProfileId::DIRECT, 1, true),
            secret("sk-corruption-test"),
        )
        .await
        .expect("create credential");
    sqlx::query(
        "UPDATE provider_credentials SET ciphertext = zeroblob(length(ciphertext)) WHERE id = ?",
    )
    .bind(credential_id.to_string())
    .execute(store.pool())
    .await
    .expect("corrupt ciphertext");

    let error = store
        .load_configuration()
        .await
        .expect_err("corrupt ciphertext must fail");
    assert!(matches!(
        error,
        StorageError::SecretVault(SecretVaultError::AuthenticationFailed)
    ));
}

fn credential_draft(
    label: &str,
    proxy_id: ProxyProfileId,
    max_concurrency: u32,
    enabled: bool,
) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        proxy_id,
        MaxConcurrency::new(max_concurrency).expect("max concurrency"),
        enabled,
    )
    .expect("credential draft")
}

fn codex_draft(name: &str, base_url: &str) -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        name,
        ProviderKind::Codex,
        base_url,
        ProtocolDialect::OpenAiResponses,
        false,
        false,
        true,
    )
    .expect("Codex endpoint draft")
}

fn claude_draft(name: &str, base_url: &str) -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        name,
        ProviderKind::Claude,
        base_url,
        ProtocolDialect::AnthropicMessages,
        false,
        false,
        true,
    )
    .expect("Claude endpoint draft")
}

fn proxy_draft() -> ProxyDraft {
    ProxyDraft::new(
        "Hong Kong",
        ProxyKind::Http,
        ProxyAddress::new("proxy.example.com", 8080).expect("proxy address"),
        true,
    )
    .expect("proxy draft")
}

fn secret(value: &str) -> SecretBytes {
    value.as_bytes().to_vec().into()
}

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderEndpointValidationError, ProviderKind,
    ProxyAddress, ProxyDraft, ProxyKind, ProxyProfileId, RequestsPerMinute,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationMutation, ConfigurationRepository, SecretBytes, SqliteStore},
    configuration::commit_configuration,
    error::StorageError,
};

mod integrity;

#[tokio::test]
async fn credential_lifecycle_persists_versions_and_secret_metadata() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();

    let endpoint = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: codex_draft("Codex Primary", "https://api.example.com"),
        },
    )
    .await
    .expect("create endpoint");
    let created = commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: credential_draft("Primary", ProxyProfileId::DIRECT, Some(40), true),
            api_key: secret("sk-first-credential"),
        },
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
    assert_eq!(
        credential.requests_per_minute().map(|value| value.get()),
        Some(40)
    );
    assert_eq!(credential.fingerprint().tail(), Some("tial"));
    assert_eq!(
        created
            .provider_credential_secrets()
            .get(credential_id)
            .expect("created secret material")
            .expose_for_test(),
        b"sk-first-credential"
    );
    assert!(!format!("{created:?}").contains("sk-first-credential"));

    let no_op = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::UpdateProviderCredential {
            id: credential_id,
            expected_config_version: 1,
            draft: credential_draft("Primary", ProxyProfileId::DIRECT, Some(40), true),
        },
    )
    .await
    .expect("no-op update");
    assert_eq!(no_op.revision(), created.revision());

    let disabled = commit_configuration(
        &store,
        no_op.revision(),
        ConfigurationMutation::UpdateProviderCredential {
            id: credential_id,
            expected_config_version: 1,
            draft: credential_draft("Primary", ProxyProfileId::DIRECT, None, false),
        },
    )
    .await
    .expect("disable credential");
    let disabled_credential = disabled
        .provider_credentials()
        .get(credential_id)
        .expect("disabled credential");
    assert_eq!(disabled_credential.config_version(), 2);
    assert_eq!(disabled_credential.credential_generation(), 1);
    assert_eq!(disabled_credential.requests_per_minute(), None);

    let enabled = commit_configuration(
        &store,
        disabled.revision(),
        ConfigurationMutation::UpdateProviderCredential {
            id: credential_id,
            expected_config_version: 2,
            draft: credential_draft("Primary", ProxyProfileId::DIRECT, Some(80), true),
        },
    )
    .await
    .expect("enable credential");
    let rotated = commit_configuration(
        &store,
        enabled.revision(),
        ConfigurationMutation::RotateProviderCredentialSecret {
            id: credential_id,
            expected_config_version: 3,
            expected_secret_version: 1,
            api_key: secret("sk-second-rotated"),
        },
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
    assert_eq!(
        rotated
            .provider_credential_secrets()
            .get(credential_id)
            .expect("rotated secret material")
            .expose_for_test(),
        b"sk-second-rotated"
    );
    assert!(!format!("{rotated:?}").contains("sk-second-rotated"));

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
    assert_eq!(
        restored
            .provider_credential_secrets()
            .get(credential_id)
            .expect("restored secret material")
            .expose_for_test(),
        b"sk-second-rotated"
    );
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

    let proxy = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProxy {
            id: proxy_id,
            draft: proxy_draft(),
        },
    )
    .await
    .expect("create proxy");
    let endpoint = commit_configuration(
        &store,
        proxy.revision(),
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: codex_draft("Codex Primary", "https://api.example.com"),
        },
    )
    .await
    .expect("create endpoint");
    let created = commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: credential_draft("Primary", proxy_id, Some(20), true),
            api_key: secret("sk-reference-test"),
        },
    )
    .await
    .expect("create credential");

    assert!(matches!(
        commit_configuration(
            &store,
            created.revision(),
            ConfigurationMutation::DeleteProxy { id: proxy_id },
        )
        .await
        .expect_err("referenced proxy must be protected"),
        StorageError::ProxyReferenced
    ));
    assert!(matches!(
        commit_configuration(
            &store,
            created.revision(),
            ConfigurationMutation::DeleteProviderEndpoint { id: endpoint_id },
        )
        .await
        .expect_err("referenced endpoint must be protected"),
        StorageError::ProviderEndpointInUse
    ));
    assert!(matches!(
        commit_configuration(
            &store,
            created.revision(),
            ConfigurationMutation::UpdateProviderEndpoint {
                id: endpoint_id,
                expected_config_version: 1,
                draft: claude_draft("Codex Primary", "https://api.anthropic.com"),
            },
        )
        .await
        .expect_err("provider identity must stay stable"),
        StorageError::ProviderEndpointValidation(
            ProviderEndpointValidationError::ProviderKindChanged
        )
    ));

    let moved = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::UpdateProviderEndpoint {
            id: endpoint_id,
            expected_config_version: 1,
            draft: codex_draft("Codex Primary", "https://edge.example.com"),
        },
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

    let endpoint = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: codex_draft("Codex Primary", "https://api.example.com"),
        },
    )
    .await
    .expect("create endpoint");
    let created = commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: first_id,
            endpoint_id,
            draft: credential_draft("Primary", ProxyProfileId::DIRECT, Some(10), true),
            api_key: secret("sk-conflict-first"),
        },
    )
    .await
    .expect("create credential");

    assert!(matches!(
        commit_configuration(
            &store,
            created.revision(),
            ConfigurationMutation::CreateProviderCredential {
                id: CredentialId::new(),
                endpoint_id,
                draft: credential_draft("primary", ProxyProfileId::DIRECT, Some(10), true),
                api_key: secret("sk-conflict-second"),
            },
        )
        .await
        .expect_err("duplicate label must fail"),
        StorageError::ProviderCredentialLabelConflict
    ));
    assert!(matches!(
        commit_configuration(
            &store,
            created.revision(),
            ConfigurationMutation::RotateProviderCredentialSecret {
                id: first_id,
                expected_config_version: 1,
                expected_secret_version: 2,
                api_key: secret("sk-stale-secret"),
            },
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

fn credential_draft(
    label: &str,
    proxy_id: ProxyProfileId,
    requests_per_minute: Option<u32>,
    enabled: bool,
) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        proxy_id,
        requests_per_minute.map(|value| RequestsPerMinute::new(value).expect("valid RPM")),
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

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyProfileId, PublicModelName,
};
use tempfile::tempdir;

mod cascade;

use crate::{
    api::{ConfigurationMutation, ConfigurationRepository, SecretBytes, SqliteStore},
    configuration::commit_configuration,
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

    let created = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id,
            draft: codex_draft("https://api.example.com/v1/"),
        },
    )
    .await
    .expect("create endpoint");
    let no_op = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::UpdateProviderEndpoint {
            id,
            expected_config_version: 1,
            draft: codex_draft("https://api.example.com/v1"),
        },
    )
    .await
    .expect("no-op update");
    let updated = commit_configuration(
        &store,
        no_op.revision(),
        ConfigurationMutation::UpdateProviderEndpoint {
            id,
            expected_config_version: 1,
            draft: codex_draft("https://edge.example.com/openai"),
        },
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

    let stale = commit_configuration(
        &store,
        updated.revision(),
        ConfigurationMutation::UpdateProviderEndpoint {
            id,
            expected_config_version: 1,
            draft: codex_draft("https://stale.example.com"),
        },
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

    let deleted = commit_configuration(
        &store,
        updated.revision(),
        ConfigurationMutation::DeleteProviderEndpoint { id },
    )
    .await
    .expect("delete endpoint");
    assert_eq!(deleted.revision().get(), 4);
    assert!(deleted.provider_endpoints().endpoints().is_empty());
}

#[tokio::test]
async fn accepted_and_optional_upstream_protocols_round_trip_without_storage_pair_rules() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let bridged_id = ProviderEndpointId::new();
    let direct_id = ProviderEndpointId::new();

    let bridged = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: bridged_id,
            draft: ProviderEndpointDraft::new(
                "Responses through Chat",
                ProviderKind::Codex,
                "https://api.example.com/v1",
                ProtocolDialect::OpenAiResponses,
                Some(ProtocolDialect::OpenAiChatCompletions),
                true,
            )
            .expect("bridged draft"),
        },
    )
    .await
    .expect("bridged endpoint");
    let direct = commit_configuration(
        &store,
        bridged.revision(),
        ConfigurationMutation::CreateProviderEndpoint {
            id: direct_id,
            draft: ProviderEndpointDraft::new(
                "Direct Chat",
                ProviderKind::Codex,
                "https://chat.example.com/v1",
                ProtocolDialect::OpenAiChatCompletions,
                None,
                true,
            )
            .expect("direct draft"),
        },
    )
    .await
    .expect("direct endpoint");

    let bridged = direct
        .provider_endpoints()
        .get(bridged_id)
        .expect("bridged endpoint");
    assert_eq!(
        bridged.upstream_protocol_dialect(),
        Some(ProtocolDialect::OpenAiChatCompletions)
    );
    let direct = direct
        .provider_endpoints()
        .get(direct_id)
        .expect("direct endpoint");
    assert_eq!(
        direct.protocol_dialect(),
        ProtocolDialect::OpenAiChatCompletions
    );
    assert_eq!(direct.upstream_protocol_dialect(), None);
}

#[tokio::test]
async fn protocol_change_with_credentials_rebuilds_routes_and_bumps_generation() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();

    let endpoint = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: codex_draft("https://api.example.com"),
        },
    )
    .await
    .expect("create endpoint");
    let credential = commit_configuration(
        &store,
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
            api_key: SecretBytes::from(b"sk-protocol-change".to_vec()),
        },
    )
    .await
    .expect("create credential");
    let modeled = commit_configuration(
        &store,
        credential.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version: 1,
            models: vec!["gpt-protocol-change".to_owned()],
        },
    )
    .await
    .expect("set credential models");

    let changed = commit_configuration(
        &store,
        modeled.revision(),
        ConfigurationMutation::UpdateProviderEndpoint {
            id: endpoint_id,
            expected_config_version: 1,
            draft: chat_draft("https://api.example.com"),
        },
    )
    .await
    .expect("change accepted protocol with an existing credential");
    let public_model = PublicModelName::new("gpt-protocol-change").expect("public model");

    assert_eq!(
        changed
            .provider_credentials()
            .get(credential_id)
            .expect("credential")
            .credential_generation(),
        2
    );
    assert!(
        changed
            .model_routes()
            .resolve(ProtocolDialect::OpenAiResponses, &public_model)
            .is_none()
    );
    assert!(
        changed
            .model_routes()
            .resolve(ProtocolDialect::OpenAiChatCompletions, &public_model)
            .is_some()
    );
}

#[tokio::test]
async fn openai_compatible_provider_kinds_round_trip_as_api_key_providers() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let mut revision = ConfigRevision::INITIAL;
    for (kind, name, url, dialect) in [
        (
            ProviderKind::Grok,
            "Grok Primary",
            "https://api.x.ai/v1",
            ProtocolDialect::OpenAiResponses,
        ),
        (
            ProviderKind::Kimi,
            "Kimi Primary",
            "https://api.moonshot.cn/v1",
            ProtocolDialect::OpenAiChatCompletions,
        ),
    ] {
        let id = ProviderEndpointId::new();
        let published = commit_configuration(
            &store,
            revision,
            ConfigurationMutation::CreateProviderEndpoint {
                id,
                draft: ProviderEndpointDraft::new(name, kind, url, dialect, None, true)
                    .expect("provider draft"),
            },
        )
        .await
        .expect("create provider endpoint");
        let endpoint = published
            .provider_endpoints()
            .get(id)
            .expect("stored provider endpoint");
        assert_eq!(endpoint.provider_kind(), kind);
        assert_eq!(endpoint.base_url().as_str(), url);
        revision = published.revision();
    }
}

#[tokio::test]
async fn duplicate_endpoint_names_are_rejected_before_commit() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let first = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: ProviderEndpointId::new(),
            draft: codex_draft("https://api.example.com"),
        },
    )
    .await
    .expect("first endpoint");

    let error = commit_configuration(
        &store,
        first.revision(),
        ConfigurationMutation::CreateProviderEndpoint {
            id: ProviderEndpointId::new(),
            draft: codex_draft("https://edge.example.com"),
        },
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
         (id, name, name_key, provider_kind, base_url, protocol_dialect, enabled, config_version) \
         VALUES (?, 'Unsafe', 'unsafe', 'codex', 'ftp://provider.example.com', \
                 'openai_responses', 1, 1)",
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

fn chat_draft(base_url: &str) -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        base_url,
        ProtocolDialect::OpenAiChatCompletions,
        None,
        true,
    )
    .expect("Chat Completions endpoint draft")
}

fn codex_draft(base_url: &str) -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        base_url,
        ProtocolDialect::OpenAiResponses,
        None,
        true,
    )
    .expect("endpoint draft")
}

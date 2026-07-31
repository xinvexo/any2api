use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ModelAccess, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId, PublicModelName, RateLimitMode, RequestsPerMinute, SettingKey, SettingValue,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigPublishError, ConfigPublisher, PublishedSnapshot, SnapshotStore},
    credential::ProviderApiKeySecret,
    registry::RuntimeRegistry,
    routing::{QueuePolicy, RateLimitAction},
};

#[tokio::test]
async fn snapshots_reuse_queue_state_but_capture_policy_per_revision() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial_configuration = storage
        .load_configuration()
        .await
        .expect("initial configuration");
    let initial_policy =
        QueuePolicy::from_scheduler_settings(initial_configuration.settings().scheduler());
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial_configuration,
        runtime.as_ref(),
        crate::test_support::configuration_capabilities().provider_registry(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let first = snapshots.load();
    let coordinator = Arc::clone(first.queue_coordinator());
    let ticket = coordinator.try_ticket(128).expect("waiting ticket");

    assert_eq!(first.queue_policy(), initial_policy);
    let second = publisher
        .set_setting_override(
            first.revision(),
            SettingKey::SchedulerOnRateLimited,
            SettingValue::RateLimitMode(RateLimitMode::Reject),
        )
        .await
        .expect("publish next snapshot");

    assert_eq!(
        second.queue_policy().on_rate_limited(),
        RateLimitAction::Reject
    );
    assert!(second.revision() > first.revision());
    assert!(Arc::ptr_eq(second.queue_coordinator(), &coordinator));
    assert_eq!(runtime.queue_waiting_count(), 1);

    drop(ticket);
    assert_eq!(runtime.queue_waiting_count(), 0);
}

#[tokio::test]
async fn published_rpm_update_preserves_the_stable_runtime_window() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial,
        runtime.as_ref(),
        crate::test_support::configuration_capabilities().provider_registry(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, codex_endpoint_draft())
        .await
        .expect("endpoint");
    let before_update = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(1),
            ProviderApiKeySecret::new("sk-settings-runtime".to_owned()),
        )
        .await
        .expect("credential");
    let before_binding = before_update
        .credential_runtime(credential_id.into())
        .expect("credential runtime");
    let first = before_binding.try_reserve().expect("first RPM reservation");
    let generation = Arc::clone(before_binding.generation());
    let epoch_before_publish = runtime.scheduler_epoch();

    let after_update = publisher
        .update_provider_credential(
            before_update.revision(),
            credential_id,
            1,
            credential_draft(2),
        )
        .await
        .expect("publish RPM update");
    let after_binding = after_update
        .credential_runtime(credential_id.into())
        .expect("updated credential runtime");

    assert!(Arc::ptr_eq(&generation, after_binding.generation()));
    assert_eq!(after_binding.rate_snapshot().requests_per_minute(), Some(2));
    assert_eq!(after_binding.rate_snapshot().requests_in_window(), 1);
    let second = after_binding
        .try_reserve()
        .expect("raised RPM allows one more reservation");
    assert!(after_binding.try_reserve().is_err());
    assert_eq!(runtime.scheduler_epoch(), epoch_before_publish + 1);
    drop((first, second));
    assert_eq!(after_binding.rate_snapshot().requests_in_window(), 2);
}

#[tokio::test]
async fn model_allowlist_filters_the_snapshot_and_prunes_removed_routes() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial,
        runtime.as_ref(),
        crate::test_support::configuration_capabilities().provider_registry(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, codex_endpoint_draft())
        .await
        .expect("endpoint");
    let credential = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(10),
            ProviderApiKeySecret::new("sk-model-allowlist".to_owned()),
        )
        .await
        .expect("credential");
    let modeled = publisher
        .set_provider_credential_models(
            credential.revision(),
            credential_id,
            1,
            vec!["gpt-a".to_owned(), "gpt-z".to_owned()],
        )
        .await
        .expect("models");
    assert_eq!(
        modeled.public_model_names().into_iter().collect::<Vec<_>>(),
        ["gpt-a", "gpt-z"]
    );

    let filtered = publisher
        .set_setting_override(
            modeled.revision(),
            SettingKey::ModelsAllowed,
            SettingValue::ModelAccess(ModelAccess::Allowlist(vec!["gpt-z".to_owned()])),
        )
        .await
        .expect("allowlist");
    assert_eq!(
        filtered
            .published_public_model_names()
            .into_iter()
            .collect::<Vec<_>>(),
        ["gpt-a", "gpt-z"]
    );
    assert_eq!(
        filtered
            .public_model_names()
            .into_iter()
            .collect::<Vec<_>>(),
        ["gpt-z"]
    );
    assert!(filtered.is_public_model_allowed(&PublicModelName::new("gpt-z").expect("model")));
    assert!(!filtered.is_public_model_allowed(&PublicModelName::new("gpt-a").expect("model")));

    let denied = publisher
        .set_setting_override(
            filtered.revision(),
            SettingKey::ModelsAllowed,
            SettingValue::ModelAccess(ModelAccess::Allowlist(Vec::new())),
        )
        .await
        .expect("deny every model");
    assert_eq!(
        denied
            .published_public_model_names()
            .into_iter()
            .collect::<Vec<_>>(),
        ["gpt-a", "gpt-z"]
    );
    assert!(denied.public_model_names().is_empty());
    assert!(!denied.is_public_model_allowed(&PublicModelName::new("gpt-z").expect("model")));

    let filtered = publisher
        .set_setting_override(
            denied.revision(),
            SettingKey::ModelsAllowed,
            SettingValue::ModelAccess(ModelAccess::Allowlist(vec!["gpt-z".to_owned()])),
        )
        .await
        .expect("restore filtered allowlist");

    let unavailable = publisher
        .set_setting_override(
            filtered.revision(),
            SettingKey::ModelsAllowed,
            SettingValue::ModelAccess(ModelAccess::Allowlist(vec!["missing".to_owned()])),
        )
        .await
        .expect_err("unpublished selection");
    assert!(matches!(unavailable, ConfigPublishError::InvalidSetting(_)));

    let deleted = publisher
        .delete_provider_credential(filtered.revision(), credential_id, 2)
        .await
        .expect("delete credential");
    assert!(deleted.published_public_model_names().is_empty());
    assert!(
        !deleted
            .settings()
            .models()
            .allows(&PublicModelName::new("gpt-after-prune").expect("public model"))
    );
    assert_eq!(
        deleted.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(
            ModelAccess::Allowlist(Vec::new())
        ))
    );
}

fn codex_endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com",
        ProtocolDialect::OpenAiResponses,
        true,
    )
    .expect("endpoint draft")
}

fn credential_draft(requests_per_minute: u32) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        Some(RequestsPerMinute::new(requests_per_minute).expect("valid RPM")),
        true,
    )
    .expect("credential draft")
}

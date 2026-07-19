use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId, SaturationMode, SettingKey, SettingValue,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

use crate::{
    auxiliary_scheduler::AuxiliarySelectAndAcquireResult,
    provider_api_key_secret::ProviderApiKeySecret,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    queue::{QueuePolicy, SaturationAction},
    registry::RuntimeRegistry,
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
    let runtime = Arc::new(RuntimeRegistry::new(
        initial_configuration.settings().scheduler(),
    ));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial_configuration,
        runtime.as_ref(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    );
    let first = snapshots.load();
    let coordinator = Arc::clone(first.queue_coordinator());
    let ticket = coordinator.try_ticket(128).expect("waiting ticket");

    assert_eq!(first.queue_policy(), initial_policy);
    let second = publisher
        .set_setting_override(
            first.revision(),
            SettingKey::SchedulerOnSaturated,
            SettingValue::Saturation(SaturationMode::Reject),
        )
        .await
        .expect("publish next snapshot");

    assert_eq!(
        second.queue_policy().on_saturated(),
        SaturationAction::Reject
    );
    assert!(second.revision() > first.revision());
    assert!(Arc::ptr_eq(second.queue_coordinator(), &coordinator));
    assert_eq!(runtime.queue_waiting_count(), 1);

    drop(ticket);
    assert_eq!(runtime.queue_waiting_count(), 0);
}

#[tokio::test]
async fn published_auxiliary_limit_update_preserves_permits_and_scheduler_identity() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new(initial.settings().scheduler()));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial,
        runtime.as_ref(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    );
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, codex_endpoint_draft())
        .await
        .expect("endpoint");
    let before_settings = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(),
            ProviderApiKeySecret::new("sk-settings-runtime".to_owned()),
        )
        .await
        .expect("credential");
    let scheduler = Arc::clone(before_settings.auxiliary_scheduler());
    let first = acquire_auxiliary(&scheduler, before_settings.as_ref());
    let second = acquire_auxiliary(&scheduler, before_settings.as_ref());
    let epoch_before_publish = runtime.scheduler_epoch();
    let mut epoch = runtime.subscribe_scheduler_epoch();
    epoch.borrow_and_update();

    let after_settings = publisher
        .set_setting_override(
            before_settings.revision(),
            SettingKey::SchedulerAuxiliaryGlobalConcurrency,
            SettingValue::Integer(1),
        )
        .await
        .expect("publish auxiliary limit");
    epoch.changed().await.expect("setting publication epoch");

    assert!(Arc::ptr_eq(
        &scheduler,
        after_settings.auxiliary_scheduler()
    ));
    assert_eq!(snapshots.load().revision(), after_settings.revision());
    assert_eq!(scheduler.global_in_flight(), 2);
    assert_eq!(runtime.auxiliary_limits().global(), 1);
    assert_eq!(runtime.scheduler_epoch(), epoch_before_publish + 1);
    assert!(matches!(
        scheduler.select_index_and_try_acquire(after_settings.credential_runtimes(), 0),
        AuxiliarySelectAndAcquireResult::AtCapacity
    ));

    drop(first);
    assert!(matches!(
        scheduler.select_index_and_try_acquire(after_settings.credential_runtimes(), 0),
        AuxiliarySelectAndAcquireResult::AtCapacity
    ));
    drop(second);
    let after_drain = acquire_auxiliary(&scheduler, after_settings.as_ref());
    drop(after_drain);
}

fn acquire_auxiliary(
    scheduler: &Arc<crate::auxiliary_scheduler::AuxiliaryScheduler>,
    snapshot: &PublishedSnapshot,
) -> crate::auxiliary_scheduler::AuxiliaryPermit {
    match scheduler.select_index_and_try_acquire(snapshot.credential_runtimes(), 0) {
        AuxiliarySelectAndAcquireResult::Acquired { permit, .. } => permit,
        AuxiliarySelectAndAcquireResult::AtCapacity => panic!("auxiliary capacity available"),
        AuxiliarySelectAndAcquireResult::NoCandidates => panic!("test configured a credential"),
    }
}

fn codex_endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com",
        ProtocolDialect::OpenAiResponses,
        false,
        false,
        true,
    )
    .expect("endpoint draft")
}

fn credential_draft() -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        MaxConcurrency::new(4).expect("max concurrency"),
        true,
    )
    .expect("credential draft")
}

use std::sync::Arc;

use any2api_domain::{ConfigRevision, ProxyAddress, ProxyDraft, ProxyKind, ProxyProfileId};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

use crate::{
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
    let initial_policy = QueuePolicy::new(
        SaturationAction::Wait,
        std::time::Duration::from_secs(1),
        1,
        false,
    )
    .expect("initial policy");
    let runtime = Arc::new(RuntimeRegistry::with_queue_policy(initial_policy));
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
    let ticket = coordinator.try_ticket(1).expect("waiting ticket");

    let updated_policy = QueuePolicy::new(
        SaturationAction::Reject,
        std::time::Duration::from_secs(5),
        4,
        true,
    )
    .expect("updated policy");

    assert_eq!(first.queue_policy(), initial_policy);
    publisher
        .create_proxy(
            ConfigRevision::INITIAL,
            ProxyProfileId::new(),
            ProxyDraft::new(
                "Queue test proxy",
                ProxyKind::Socks5,
                ProxyAddress::new("proxy.example.com", 1080).expect("proxy address"),
                true,
            )
            .expect("proxy draft"),
        )
        .await
        .expect("publish next snapshot");
    let next_configuration = storage
        .load_configuration()
        .await
        .expect("next configuration");
    let second = PublishedSnapshot::new_with_queue_policy(
        next_configuration,
        runtime.as_ref(),
        updated_policy,
    );

    assert_eq!(second.queue_policy(), updated_policy);
    assert!(second.revision() > first.revision());
    assert!(Arc::ptr_eq(second.queue_coordinator(), &coordinator));
    assert_eq!(runtime.queue_waiting_count(), 1);

    drop(ticket);
    assert_eq!(runtime.queue_waiting_count(), 0);
}

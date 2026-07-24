use std::sync::Arc;

use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::tempdir;

use crate::{published_snapshot::PublishedSnapshot, registry::RuntimeRegistry};

#[tokio::test]
async fn fresh_runtime_snapshot_reports_compiled_queue_and_empty_capacity() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = RuntimeRegistry::new(configuration.settings().scheduler());
    let capabilities = crate::test_support::configuration_capabilities();
    let published =
        PublishedSnapshot::new(configuration, &runtime, capabilities.provider_registry());
    let snapshot = runtime.balancing_snapshot(&published);

    assert_eq!(snapshot.scheduler_epoch(), 0);
    assert_eq!(snapshot.queue().waiting(), 0);
    assert_eq!(snapshot.queue().max_waiting(), 128);
    assert_eq!(snapshot.queue().timeout_secs(), 30);
    assert!(!snapshot.queue().rejects_when_saturated());
    assert!(!snapshot.queue().fallback_on_saturation());
    assert_eq!(snapshot.auxiliary().in_flight(), 0);
    assert_eq!(snapshot.auxiliary().max_global(), 32);
    assert_eq!(snapshot.auxiliary().max_per_credential(), 4);
    assert!(snapshot.credentials().is_empty());
}

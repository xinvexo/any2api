use std::sync::{Arc, Mutex};

use any2api_domain::{ConfigRevision, LoggingSettings, ProxyProfileId};

use super::{TestContext, proxy_draft};
use crate::configuration::{ConfigPublisher, LoggingSettingsReconciler};

#[tokio::test]
async fn logging_reconcile_observes_the_already_published_revision() {
    let context = TestContext::new().await;
    let observations = Arc::new(Mutex::new(Vec::new()));
    let reconciler = Arc::new(RevisionObserver {
        snapshots: Arc::clone(&context.snapshots),
        observations: Arc::clone(&observations),
    });
    let publisher = ConfigPublisher::new(
        Arc::clone(&context.repository),
        Arc::clone(&context.snapshots),
        Arc::clone(&context.runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher")
    .with_logging_reconciler(reconciler);

    let published = publisher
        .create_proxy(
            ConfigRevision::INITIAL,
            ProxyProfileId::new(),
            proxy_draft("ordered logging"),
        )
        .await
        .expect("publish proxy");

    assert_eq!(
        *observations.lock().expect("observation lock poisoned"),
        vec![(published.revision(), published.revision())]
    );
}

struct RevisionObserver {
    snapshots: Arc<crate::configuration::SnapshotStore>,
    observations: Arc<Mutex<Vec<(ConfigRevision, ConfigRevision)>>>,
}

impl LoggingSettingsReconciler for RevisionObserver {
    fn reconcile(&self, revision: ConfigRevision, _settings: &LoggingSettings) {
        self.observations
            .lock()
            .expect("observation lock poisoned")
            .push((revision, self.snapshots.load().revision()));
    }
}

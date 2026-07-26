use std::{sync::Arc, time::Duration};

use crate::{configuration::ConfigPublisher, lifecycle::ProcessLifecycle};

use super::registry::AffinityRegistry;

const SWEEP_INTERVAL: Duration = Duration::from_secs(60);

/// Periodically drops expired bindings so idle sessions do not pin memory
/// until the next admin snapshot or capacity-triggered cleanup.
pub(crate) fn start(
    registry: Arc<AffinityRegistry>,
    publisher: Arc<ConfigPublisher>,
    lifecycle: &ProcessLifecycle,
) {
    drop(lifecycle.spawn_until_draining(async move {
        loop {
            tokio::time::sleep(SWEEP_INTERVAL).await;
            let policy = publisher.current_snapshot().affinity_policy();
            let removed = registry.sweep_expired(
                policy.soft_ttl(),
                policy.hard_ttl(),
                policy.fixed_wait_timeout(),
            );
            if removed > 0 {
                tracing::debug!(removed, "expired affinity bindings swept");
            }
        }
    }));
}

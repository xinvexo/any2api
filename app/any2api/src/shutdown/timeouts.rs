use std::time::Duration;

use any2api_domain::{SettingsConfiguration, ShutdownSettings};
use any2api_runtime::api::SnapshotStore;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShutdownTimeouts {
    pub(super) request_grace: Duration,
    pub(super) finalize: Duration,
}

impl ShutdownTimeouts {
    pub(crate) fn capture(snapshots: &SnapshotStore) -> Self {
        let snapshot = snapshots.load();
        Self::from_settings(snapshot.settings().shutdown())
    }

    pub(crate) fn defaults() -> Self {
        Self::from_settings(SettingsConfiguration::defaults().shutdown())
    }

    pub(crate) fn runtime_shutdown_timeout(self) -> Duration {
        self.finalize
    }

    pub(super) fn from_settings(settings: &ShutdownSettings) -> Self {
        Self {
            request_grace: Duration::from_secs(settings.request_grace_period_secs()),
            finalize: Duration::from_secs(settings.finalize_timeout_secs()),
        }
    }
}

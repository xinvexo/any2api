use any2api_domain::{ConfigRevision, LoggingSettings};

pub trait LoggingSettingsReconciler: Send + Sync {
    fn reconcile(&self, revision: ConfigRevision, settings: &LoggingSettings);
}

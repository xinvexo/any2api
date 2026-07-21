use std::sync::Arc;

use any2api_domain::{ConfigRevision, LoggingSettings};
use any2api_runtime::api::{LoggingSettingsReconciler, RequestTelemetry};

use crate::file_logging::FileLogging;

pub(crate) struct AppLoggingReconciler {
    telemetry: Arc<RequestTelemetry>,
    file_logging: Arc<FileLogging>,
}

impl AppLoggingReconciler {
    pub(crate) fn new(telemetry: Arc<RequestTelemetry>, file_logging: Arc<FileLogging>) -> Self {
        Self {
            telemetry,
            file_logging,
        }
    }
}

impl LoggingSettingsReconciler for AppLoggingReconciler {
    fn reconcile(&self, revision: ConfigRevision, settings: &LoggingSettings) {
        self.telemetry.reconcile(revision, settings);
        self.file_logging.reconcile(revision, settings);
    }
}

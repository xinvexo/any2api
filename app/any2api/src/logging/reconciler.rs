use std::sync::Arc;

use any2api_runtime::api::{PublishedSnapshot, PublishedSnapshotReconciler, RequestTelemetry};

use super::file::FileLoggingControl;

pub(crate) struct AppSnapshotReconciler {
    telemetry: Arc<RequestTelemetry>,
    file_logging: FileLoggingControl,
}

impl AppSnapshotReconciler {
    pub(crate) fn new(telemetry: Arc<RequestTelemetry>, file_logging: FileLoggingControl) -> Self {
        Self {
            telemetry,
            file_logging,
        }
    }
}

impl PublishedSnapshotReconciler for AppSnapshotReconciler {
    fn reconcile(&self, snapshot: &PublishedSnapshot) {
        self.telemetry.reconcile(snapshot);
        self.file_logging.reconcile(snapshot);
    }
}

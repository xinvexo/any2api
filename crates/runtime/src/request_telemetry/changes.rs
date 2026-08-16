use tokio::sync::watch;

use super::telemetry::RequestTelemetry;

#[derive(Clone)]
pub(super) struct LogChangeNotifier {
    request_logs: watch::Sender<u64>,
    active_requests: watch::Sender<u64>,
    http_access_logs: watch::Sender<u64>,
}

impl LogChangeNotifier {
    pub(super) fn new() -> Self {
        let (request_logs, _) = watch::channel(0);
        let (active_requests, _) = watch::channel(0);
        let (http_access_logs, _) = watch::channel(0);
        Self {
            request_logs,
            active_requests,
            http_access_logs,
        }
    }

    pub(super) fn request_logs_changed(&self) {
        self.request_logs
            .send_modify(|epoch| *epoch = epoch.saturating_add(1));
    }

    pub(super) fn http_access_logs_changed(&self) {
        self.http_access_logs
            .send_modify(|epoch| *epoch = epoch.saturating_add(1));
    }

    pub(super) fn active_requests_changed(&self) {
        self.active_requests
            .send_modify(|epoch| *epoch = epoch.saturating_add(1));
    }

    pub(super) fn subscribe_request_logs(&self) -> watch::Receiver<u64> {
        self.request_logs.subscribe()
    }

    pub(super) fn subscribe_http_access_logs(&self) -> watch::Receiver<u64> {
        self.http_access_logs.subscribe()
    }

    pub(super) fn subscribe_active_requests(&self) -> watch::Receiver<u64> {
        self.active_requests.subscribe()
    }
}

impl RequestTelemetry {
    #[must_use]
    pub fn subscribe_request_log_changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe_request_logs()
    }

    #[must_use]
    pub fn subscribe_http_access_log_changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe_http_access_logs()
    }
}

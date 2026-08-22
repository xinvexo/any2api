use any2api_domain::{ConfigRevision, LoggingSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RequestLogPolicy {
    pub(crate) revision: ConfigRevision,
    pub(crate) enabled: bool,
    pub(crate) retention_secs: u64,
    pub(crate) request_max_rows: u64,
    pub(crate) http_access_max_rows: u64,
    pub(crate) queue_capacity: usize,
    pub(crate) queue_max_bytes: usize,
}

impl RequestLogPolicy {
    pub(crate) fn from_settings(revision: ConfigRevision, settings: &LoggingSettings) -> Self {
        Self {
            revision,
            enabled: settings.request_enabled(),
            retention_secs: settings.request_retention_secs(),
            request_max_rows: settings.request_max_rows(),
            http_access_max_rows: settings.http_access_max_rows(),
            queue_capacity: usize::try_from(settings.telemetry_queue_capacity())
                .expect("validated telemetry queue capacity fits usize"),
            queue_max_bytes: usize::try_from(settings.telemetry_queue_max_bytes())
                .unwrap_or(usize::MAX),
        }
    }

    pub(crate) const fn cleanup_limits_differ(self, other: Self) -> bool {
        self.retention_secs != other.retention_secs
            || self.request_max_rows != other.request_max_rows
            || self.http_access_max_rows != other.http_access_max_rows
    }
}

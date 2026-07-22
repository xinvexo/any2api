use any2api_domain::{ConfigRevision, LoggingSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RequestLogPolicy {
    pub(crate) revision: ConfigRevision,
    pub(crate) enabled: bool,
    pub(crate) retention_secs: u64,
    pub(crate) max_rows: u64,
    pub(crate) queue_capacity: usize,
}

impl RequestLogPolicy {
    pub(crate) fn from_settings(revision: ConfigRevision, settings: &LoggingSettings) -> Self {
        Self {
            revision,
            enabled: settings.request_enabled(),
            retention_secs: settings.request_retention_secs(),
            max_rows: settings.request_max_rows(),
            queue_capacity: usize::try_from(settings.telemetry_queue_capacity())
                .expect("validated telemetry queue capacity fits usize"),
        }
    }
}

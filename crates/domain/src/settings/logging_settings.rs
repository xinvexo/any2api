use super::{
    SettingKey, SettingOverrides, SettingsValidationError,
    value::{boolean, integer},
};

pub const MAX_REQUEST_LOG_RETENTION_MS: u64 = 365 * 24 * 60 * 60 * 1_000;
pub const MAX_REQUEST_LOG_ROWS: u64 = 10_000_000;
pub const MAX_TELEMETRY_QUEUE_CAPACITY: u64 = 100_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoggingSettings {
    request_enabled: bool,
    request_retention_ms: u64,
    request_max_rows: u64,
    telemetry_queue_capacity: u64,
}

impl LoggingSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.effective_value(key);
        Ok(Self {
            request_enabled: boolean(value(SettingKey::LogsRequestEnabled))?,
            request_retention_ms: integer(value(SettingKey::LogsRequestRetention))?,
            request_max_rows: integer(value(SettingKey::LogsRequestMaxRows))?,
            telemetry_queue_capacity: integer(value(SettingKey::LogsTelemetryQueueCapacity))?,
        })
    }

    pub const fn request_enabled(&self) -> bool {
        self.request_enabled
    }

    pub const fn request_retention_ms(&self) -> u64 {
        self.request_retention_ms
    }

    pub const fn request_max_rows(&self) -> u64 {
        self.request_max_rows
    }

    pub const fn telemetry_queue_capacity(&self) -> u64 {
        self.telemetry_queue_capacity
    }
}

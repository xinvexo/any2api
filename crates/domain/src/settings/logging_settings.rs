use super::{
    FileLogLevel, SettingKey, SettingOverrides, SettingValue, SettingsValidationError,
    value::{boolean, integer},
};

pub const MAX_REQUEST_LOG_RETENTION_SECS: u64 = 365 * 24 * 60 * 60;
pub const MAX_REQUEST_LOG_ROWS: u64 = 10_000_000;
pub const MAX_FILE_LOG_RETENTION_SECS: u64 = 365 * 24 * 60 * 60;
pub const MAX_FILE_LOG_TOTAL_SIZE: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_TELEMETRY_QUEUE_CAPACITY: u64 = 100_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoggingSettings {
    request_enabled: bool,
    request_retention_secs: u64,
    request_max_rows: u64,
    file_level: FileLogLevel,
    file_retention_secs: u64,
    file_max_total_size: u64,
    telemetry_queue_capacity: u64,
}

impl LoggingSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.effective_value(key);
        Ok(Self {
            request_enabled: boolean(value(SettingKey::LogsRequestEnabled))?,
            request_retention_secs: integer(value(SettingKey::LogsRequestRetention))?,
            request_max_rows: integer(value(SettingKey::LogsRequestMaxRows))?,
            file_level: file_log_level(value(SettingKey::LogsFileLevel))?,
            file_retention_secs: integer(value(SettingKey::LogsFileRetention))?,
            file_max_total_size: integer(value(SettingKey::LogsFileMaxTotalSize))?,
            telemetry_queue_capacity: integer(value(SettingKey::LogsTelemetryQueueCapacity))?,
        })
    }

    pub const fn request_enabled(&self) -> bool {
        self.request_enabled
    }

    pub const fn request_retention_secs(&self) -> u64 {
        self.request_retention_secs
    }

    pub const fn request_max_rows(&self) -> u64 {
        self.request_max_rows
    }

    pub const fn file_level(&self) -> FileLogLevel {
        self.file_level
    }

    pub const fn file_retention_secs(&self) -> u64 {
        self.file_retention_secs
    }

    pub const fn file_max_total_size(&self) -> u64 {
        self.file_max_total_size
    }

    pub const fn telemetry_queue_capacity(&self) -> u64 {
        self.telemetry_queue_capacity
    }
}

fn file_log_level(value: SettingValue) -> Result<FileLogLevel, SettingsValidationError> {
    match value {
        SettingValue::FileLogLevel(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

use super::{
    FileLogLevel, SettingKey, SettingOverrides, SettingValue, SettingsValidationError,
    value::{boolean, integer},
};

pub const MAX_REQUEST_LOG_RETENTION_SECS: u64 = 365 * 24 * 60 * 60;
pub const MAX_REQUEST_LOG_ROWS: u64 = 10_000_000;
pub const MAX_HTTP_ACCESS_LOG_ROWS: u64 = 10_000_000;
pub const MAX_HTTP_ACCESS_LOG_EXCHANGE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_FILE_LOG_RETENTION_SECS: u64 = 365 * 24 * 60 * 60;
pub const MAX_FILE_LOG_TOTAL_SIZE: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_TELEMETRY_QUEUE_CAPACITY: u64 = 100_000;
pub const MIN_TELEMETRY_QUEUE_MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const DEFAULT_TELEMETRY_QUEUE_MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_TELEMETRY_QUEUE_MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoggingSettings {
    request_enabled: bool,
    request_retention_secs: u64,
    request_max_rows: u64,
    http_access_max_rows: u64,
    http_access_max_exchange_bytes: u64,
    file_level: FileLogLevel,
    file_retention_secs: u64,
    file_max_total_size: u64,
    telemetry_queue_capacity: u64,
    telemetry_queue_max_bytes: u64,
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
            http_access_max_rows: integer(value(SettingKey::LogsHttpAccessMaxRows))?,
            http_access_max_exchange_bytes: integer(value(
                SettingKey::LogsHttpAccessMaxExchangeBytes,
            ))?,
            file_level: file_log_level(value(SettingKey::LogsFileLevel))?,
            file_retention_secs: integer(value(SettingKey::LogsFileRetention))?,
            file_max_total_size: integer(value(SettingKey::LogsFileMaxTotalSize))?,
            telemetry_queue_capacity: integer(value(SettingKey::LogsTelemetryQueueCapacity))?,
            telemetry_queue_max_bytes: integer(value(SettingKey::LogsTelemetryQueueMaxBytes))?,
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

    pub const fn http_access_max_rows(&self) -> u64 {
        self.http_access_max_rows
    }

    pub const fn http_access_max_exchange_bytes(&self) -> u64 {
        self.http_access_max_exchange_bytes
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

    pub const fn telemetry_queue_max_bytes(&self) -> u64 {
        self.telemetry_queue_max_bytes
    }
}

fn file_log_level(value: SettingValue) -> Result<FileLogLevel, SettingsValidationError> {
    match value {
        SettingValue::FileLogLevel(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_TELEMETRY_QUEUE_MAX_BYTES, MAX_TELEMETRY_QUEUE_MAX_BYTES,
        MIN_TELEMETRY_QUEUE_MAX_BYTES,
    };
    use crate::{SettingKey, SettingValue, SettingsConfiguration};

    #[test]
    fn telemetry_owned_byte_limit_has_one_registry_backed_default_and_range() {
        let key = SettingKey::LogsTelemetryQueueMaxBytes;
        let definition = key.definition();
        assert_eq!(
            SettingsConfiguration::defaults()
                .logging()
                .telemetry_queue_max_bytes(),
            DEFAULT_TELEMETRY_QUEUE_MAX_BYTES
        );
        assert_eq!(
            definition.default(),
            SettingValue::Integer(DEFAULT_TELEMETRY_QUEUE_MAX_BYTES)
        );
        assert_eq!(
            definition.min(),
            Some(SettingValue::Integer(MIN_TELEMETRY_QUEUE_MAX_BYTES))
        );
        assert_eq!(
            definition.max(),
            Some(SettingValue::Integer(MAX_TELEMETRY_QUEUE_MAX_BYTES))
        );
    }
}

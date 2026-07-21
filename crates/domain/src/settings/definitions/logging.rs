use crate::settings::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{definition as setting_definition, duration_definition},
    logging_settings::{
        MAX_FILE_LOG_RETENTION_MS, MAX_FILE_LOG_TOTAL_SIZE, MAX_REQUEST_LOG_RETENTION_MS,
        MAX_REQUEST_LOG_ROWS, MAX_TELEMETRY_QUEUE_CAPACITY,
    },
};

const FILE_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::LogsRequestEnabled => setting_definition(
            key,
            SettingValueType::Boolean,
            SettingValue::Boolean(true),
            (None, None),
            &[],
            (
                "请求日志",
                "把已认证的模型请求与上游 Attempt 写入本地 SQLite 历史。",
            ),
        ),
        SettingKey::LogsRequestRetention => duration_definition(
            key,
            2_592_000_000,
            60_000,
            MAX_REQUEST_LOG_RETENTION_MS,
            "请求日志",
            "RequestLog 与 Attempt 的最长本地保留时间。",
        ),
        SettingKey::LogsRequestMaxRows => integer(
            key,
            200_000,
            1,
            MAX_REQUEST_LOG_ROWS,
            "请求日志",
            "SQLite 中最多保留的 RequestLog 行数；对应 Attempt 随父记录清理。",
        ),
        SettingKey::LogsFileLevel => setting_definition(
            key,
            SettingValueType::Enum,
            SettingValue::FileLogLevel(crate::settings::FileLogLevel::Info),
            (None, None),
            FILE_LOG_LEVELS,
            ("本地文件日志", "写入本地 JSONL 文件的最低日志级别。"),
        ),
        SettingKey::LogsFileRetention => duration_definition(
            key,
            604_800_000,
            60_000,
            MAX_FILE_LOG_RETENTION_MS,
            "本地文件日志",
            "本地 JSONL 日志文件的最长保留时间。",
        ),
        SettingKey::LogsFileMaxTotalSize => integer(
            key,
            256 * 1024 * 1024,
            1024 * 1024,
            MAX_FILE_LOG_TOTAL_SIZE,
            "本地文件日志",
            "本地 JSONL 日志目录允许占用的最大总字节数。",
        ),
        SettingKey::LogsTelemetryQueueCapacity => integer(
            key,
            4_096,
            1,
            MAX_TELEMETRY_QUEUE_CAPACITY,
            "请求日志",
            "等待 SQLite Writer 消费的有界请求遥测数量；满载时直接丢弃并计数。",
        ),
        _ => unreachable!(),
    }
}

const fn integer(
    key: SettingKey,
    default: u64,
    min: u64,
    max: u64,
    group: &'static str,
    description: &'static str,
) -> SettingDefinition {
    setting_definition(
        key,
        SettingValueType::Integer,
        SettingValue::Integer(default),
        (
            Some(SettingValue::Integer(min)),
            Some(SettingValue::Integer(max)),
        ),
        &[],
        (group, description),
    )
}

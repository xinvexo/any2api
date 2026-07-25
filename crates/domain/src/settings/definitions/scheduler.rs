use crate::settings::{
    RateLimitMode, SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_SETTING_COUNT, MAX_SETTING_DURATION_SECS, definition as setting_definition,
        duration_definition,
    },
};

const ALLOWED_ACTIONS: &[&str] = &["wait", "reject"];

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::SchedulerOnRateLimited => setting_definition(
            key,
            SettingValueType::Enum,
            SettingValue::RateLimitMode(RateLimitMode::Wait),
            (None, None),
            ALLOWED_ACTIONS,
            (
                "排队策略",
                "所有可用 Credential 都用尽本地 RPM 时，等待窗口到期或立即拒绝请求。",
            ),
        ),
        SettingKey::SchedulerQueueTimeout => duration_definition(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "排队策略",
            "请求等待 RPM 名额或健康恢复的最长时间。",
        ),
        SettingKey::SchedulerMaxWaitingRequests => integer(
            key,
            128,
            1,
            MAX_SETTING_COUNT,
            "排队策略",
            "允许同时等待 RPM 名额或健康恢复的请求数量。",
        ),
        SettingKey::SchedulerFallbackOnRateLimit => setting_definition(
            key,
            SettingValueType::Boolean,
            SettingValue::Boolean(false),
            (None, None),
            &[],
            (
                "排队策略",
                "主 tier RPM 全部用尽时，是否允许继续选择 fallback tier。",
            ),
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

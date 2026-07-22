use crate::settings::{
    SaturationMode, SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_SETTING_AUXILIARY, MAX_SETTING_COUNT, MAX_SETTING_DURATION_SECS,
        definition as setting_definition, duration_definition,
    },
};

const ALLOWED_ACTIONS: &[&str] = &["wait", "reject"];

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::SchedulerOnSaturated => setting_definition(
            key,
            SettingValueType::Enum,
            SettingValue::Saturation(SaturationMode::Wait),
            (None, None),
            ALLOWED_ACTIONS,
            (
                "排队策略",
                "所有可用 Credential 都达到并发上限时，等待容量或立即拒绝请求。",
            ),
        ),
        SettingKey::SchedulerQueueTimeout => duration_definition(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "排队策略",
            "生成请求等待可用并发槽位的最长时间。",
        ),
        SettingKey::SchedulerMaxWaitingRequests => integer(
            key,
            128,
            1,
            MAX_SETTING_COUNT,
            "排队策略",
            "允许同时等待可用并发槽位的生成请求数量。",
        ),
        SettingKey::SchedulerFallbackOnSaturation => setting_definition(
            key,
            SettingValueType::Boolean,
            SettingValue::Boolean(false),
            (None, None),
            &[],
            (
                "排队策略",
                "主 tier 满载时，是否允许继续选择 fallback tier。",
            ),
        ),
        SettingKey::SchedulerAuxiliaryGlobalConcurrency => integer(
            key,
            32,
            1,
            MAX_SETTING_AUXILIARY,
            "辅助请求",
            "count_tokens 等辅助请求在整个进程内的并发上限。",
        ),
        SettingKey::SchedulerAuxiliaryPerCredentialConcurrency => integer(
            key,
            4,
            1,
            MAX_SETTING_AUXILIARY,
            "辅助请求",
            "单个 Credential 执行辅助请求的并发上限。",
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

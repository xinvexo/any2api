use crate::settings::{
    SettingDefinition, SettingKey,
    definition::{MAX_SETTING_DURATION_SECS, duration_definition},
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::RetryPrecommitTotalBudget => duration(
            key,
            600,
            1,
            MAX_SETTING_DURATION_SECS,
            "重试预算",
            "从第一次 Attempt 开始到下游提交前可用于重试和退避的总时间。",
        ),
        _ => unreachable!(),
    }
}

fn duration(
    key: SettingKey,
    default: u64,
    min: u64,
    max: u64,
    group: &'static str,
    description: &'static str,
) -> SettingDefinition {
    duration_definition(key, default, min, max, group, description)
}

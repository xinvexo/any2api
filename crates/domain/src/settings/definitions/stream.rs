use crate::settings::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_SETTING_DURATION_SECS, definition as setting_definition, duration_definition,
    },
    stream_settings::MAX_STREAM_PRECOMMIT_BYTES,
};

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::StreamPrecommitMaxBytes => integer(
            key,
            256 * 1024,
            1,
            MAX_STREAM_PRECOMMIT_BYTES,
            "流式预提交",
            "每个 SSE 帧允许的最大字节数，同时限制首个可接受事件提交前的编码后字节预算。",
        ),
        SettingKey::StreamPrecommitMaxDuration => duration_definition(
            key,
            5,
            1,
            MAX_SETTING_DURATION_SECS,
            "流式预提交",
            "等待首个可接受 SSE 事件的最长时间。",
        ),
        SettingKey::StreamPostcommitIdleTimeout => duration_definition(
            key,
            60,
            1,
            MAX_SETTING_DURATION_SECS,
            "流式响应",
            "首个事件提交后等待下一个上游 chunk 的最长空闲时间。",
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

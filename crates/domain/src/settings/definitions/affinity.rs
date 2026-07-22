use crate::settings::{
    AffinityMode, SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_AFFINITY_TTL_SECS, MAX_SETTING_DURATION_SECS, definition as setting_definition,
        duration_definition,
    },
};

const ALLOWED_MODES: &[&str] = &["prefer", "strict"];

pub(super) const fn definition_for_mode(key: SettingKey) -> SettingDefinition {
    setting_definition(
        key,
        SettingValueType::Enum,
        SettingValue::AffinityMode(AffinityMode::Prefer),
        (None, None),
        ALLOWED_MODES,
        (
            "软会话粘性",
            "prefer 在等待超时后允许重绑，strict 只允许原 Credential。",
        ),
    )
}

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::AffinitySoftEnabled => setting_definition(
            key,
            SettingValueType::Boolean,
            SettingValue::Boolean(true),
            (None, None),
            &[],
            (
                "软会话粘性",
                "是否为没有 previous_response_id 的生成请求启用软会话粘性。",
            ),
        ),
        SettingKey::AffinitySoftMode => definition_for_mode(key),
        SettingKey::AffinitySoftTtl => duration_definition(
            key,
            3_600,
            1,
            MAX_AFFINITY_TTL_SECS,
            "软会话粘性",
            "软会话绑定在当前进程内保持活跃的时间。",
        ),
        SettingKey::AffinityHardTtl => duration_definition(
            key,
            86_400,
            1,
            MAX_AFFINITY_TTL_SECS,
            "硬会话粘性",
            "Codex Response ID 与原 Credential 绑定的内存有效期。",
        ),
        SettingKey::AffinitySoftPreferWaitTimeout => duration_definition(
            key,
            2,
            1,
            MAX_SETTING_DURATION_SECS,
            "软会话粘性",
            "prefer 会话等待绑定 Credential 容量的最长时间。",
        ),
        SettingKey::AffinityFixedWaitTimeout => duration_definition(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "固定会话等待",
            "硬绑定和 strict 会话等待原 Credential 容量的最长时间。",
        ),
        _ => unreachable!(),
    }
}

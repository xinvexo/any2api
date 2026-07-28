use crate::settings::{
    SettingDefinition, SettingKey,
    definition::{MAX_AFFINITY_TTL_SECS, MAX_SETTING_DURATION_SECS, duration_definition},
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::AffinityTtl => duration_definition(
            key,
            86_400,
            1,
            MAX_AFFINITY_TTL_SECS,
            "会话粘性",
            "会话绑定在当前进程内保持活跃的时间。",
        ),
        SettingKey::AffinityWaitTimeout => duration_definition(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "会话粘性",
            "会话等待绑定创建或原 Credential 可用的最长时间。",
        ),
        _ => unreachable!(),
    }
}

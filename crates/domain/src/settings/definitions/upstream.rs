use crate::settings::{
    SettingDefinition, SettingKey,
    definition::{MAX_SETTING_DURATION_MS, duration_definition},
};

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::UpstreamReadTimeout => duration_definition(
            key,
            15_000,
            1,
            MAX_SETTING_DURATION_MS,
            "上游网络",
            "等待上游响应头或 buffered body 下一个 chunk 的最长空闲时间。",
        ),
        _ => unreachable!(),
    }
}

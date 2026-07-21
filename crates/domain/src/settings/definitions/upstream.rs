use crate::settings::{
    SettingDefinition, SettingKey, SettingValue,
    definition::{MAX_SETTING_DURATION_MS, definition as base_definition, duration_definition},
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
        SettingKey::UpstreamStrictSsrf => base_definition(
            key,
            crate::settings::SettingValueType::Boolean,
            SettingValue::Boolean(false),
            (None, None),
            &[],
            (
                "上游网络",
                "严格校验代理目标的本地 DNS 解析并固定连接地址。",
            ),
        ),
        _ => unreachable!(),
    }
}

use crate::settings::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{MAX_SETTING_DURATION_SECS, definition as setting_definition, duration_definition},
};

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::AdminRemoteEnabled => setting_definition(
            key,
            SettingValueType::Boolean,
            SettingValue::Boolean(false),
            (None, None),
            &[],
            (
                "远程管理",
                "允许非 loopback 客户端访问管理员登录和管理 API；监听地址仍由 ANY2API_BIND 决定。",
            ),
        ),
        SettingKey::AdminSessionIdleTimeout => duration_definition(
            key,
            43_200,
            60,
            2_592_000,
            "远程管理",
            "管理员会话无请求后自动失效的时间。",
        ),
        SettingKey::AdminSessionAbsoluteTimeout => duration_definition(
            key,
            604_800,
            60,
            2_592_000,
            "远程管理",
            "管理员会话从登录开始计算的绝对有效期。",
        ),
        SettingKey::AdminLoginFailureWindow => duration_definition(
            key,
            900,
            1,
            MAX_SETTING_DURATION_SECS,
            "远程管理",
            "登录失败计数保留的时间窗口。",
        ),
        SettingKey::AdminLoginMaxFailures => setting_definition(
            key,
            SettingValueType::Integer,
            SettingValue::Integer(5),
            (
                Some(SettingValue::Integer(1)),
                Some(SettingValue::Integer(100)),
            ),
            &[],
            ("远程管理", "单个来源在失败窗口内允许的最大登录失败次数。"),
        ),
        _ => unreachable!(),
    }
}

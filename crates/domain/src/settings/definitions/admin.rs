use crate::settings::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_SETTING_DURATION_SECS, definition as setting_definition, duration_definition,
    },
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::AdminRemoteEnabled => setting_definition(
            key,
            SettingValueType::Boolean,
            SettingValue::Boolean(true),
            (None, None),
            &[],
            (
                "远程管理",
                "允许其他设备打开管理页面并登录。关闭后，只有运行 any2api 的这台设备可以访问管理功能。",
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

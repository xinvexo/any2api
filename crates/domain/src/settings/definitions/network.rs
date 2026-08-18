use super::super::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_SETTING_COUNT, MAX_SETTING_DURATION_SECS, definition as setting_definition,
        duration_definition, restart_required_definition,
    },
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::NetworkMaxConnections => restart_required_definition(
            key,
            SettingValueType::Integer,
            SettingValue::Integer(4_096),
            (
                Some(SettingValue::Integer(1)),
                Some(SettingValue::Integer(MAX_SETTING_COUNT)),
            ),
            (
                "入站网络",
                "同时存活的入站 TCP 连接数量上限；保存后需要重启进程生效。",
            ),
        ),
        SettingKey::NetworkRequestHeaderTimeout => restart_required_duration(
            key,
            30,
            "HTTP 请求头读取超时；连接在请求头未完整到达前超过此时间会被关闭，保存后需要重启进程生效。",
        ),
        SettingKey::NetworkRequestBodyIdleTimeout => restart_required_duration(
            key,
            60,
            "请求体连续两个数据块之间允许的最长空闲时间，适用于公开接口和管理接口；保存后需要重启进程生效。",
        ),
        SettingKey::NetworkTrustedProxyCidrs => setting_definition(
            key,
            SettingValueType::StringList,
            SettingValue::StringList(Vec::new()),
            (None, None),
            &[],
            (
                "远程管理",
                "使用 Nginx、Caddy 等反向代理时，填写代理服务器的 IP 或网段（每行一个，也可用逗号分隔）；没有反向代理请留空。",
            ),
        ),
        _ => unreachable!(),
    }
}

fn restart_required_duration(
    key: SettingKey,
    default: u64,
    description: &'static str,
) -> SettingDefinition {
    let mut definition = duration_definition(
        key,
        default,
        1,
        MAX_SETTING_DURATION_SECS,
        "入站网络",
        description,
    );
    definition.apply_mode = super::super::SettingApplyMode::RestartRequired;
    definition
}

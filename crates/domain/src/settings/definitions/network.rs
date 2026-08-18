use super::super::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_SETTING_COUNT, definition as setting_definition, restart_required_definition,
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

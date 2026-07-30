use super::super::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::definition as setting_definition,
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
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

use super::super::{
    ModelAccess, SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::definition as setting_definition,
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::ModelsAllowed => setting_definition(
            key,
            SettingValueType::ModelAccess,
            SettingValue::ModelAccess(ModelAccess::All),
            (None, None),
            &[],
            ("公开模型", ""),
        ),
        SettingKey::ModelsFastEnabled => setting_definition(
            key,
            SettingValueType::Boolean,
            SettingValue::Boolean(true),
            (None, None),
            &[],
            (
                "公开模型",
                "允许客户端请求各协议支持的 Fast 服务档位。关闭后，显式 Fast 请求会使用标准档位。",
            ),
        ),
        _ => unreachable!(),
    }
}

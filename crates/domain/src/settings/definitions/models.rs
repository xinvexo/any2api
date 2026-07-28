use super::super::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::definition as setting_definition,
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::ModelsAllowed => setting_definition(
            key,
            SettingValueType::OptionalStringList,
            SettingValue::OptionalStringList(None),
            (None, None),
            &[],
            (
                "公开模型",
                "限制客户端可使用的公开模型；允许全部与拒绝全部使用不同的显式状态。",
            ),
        ),
        _ => unreachable!(),
    }
}

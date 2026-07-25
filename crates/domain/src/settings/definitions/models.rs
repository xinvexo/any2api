use super::super::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::definition as setting_definition,
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::ModelsAllowed => setting_definition(
            key,
            SettingValueType::StringList,
            SettingValue::StringList(Vec::new()),
            (None, None),
            &[],
            (
                "公开模型",
                "限制客户端可使用的公开模型；未选择时允许全部当前模型。",
            ),
        ),
        _ => unreachable!(),
    }
}

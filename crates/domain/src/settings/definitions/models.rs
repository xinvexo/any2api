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
        _ => unreachable!(),
    }
}

use super::{SettingKey, SettingOverrides, SettingValue, SettingsValidationError};
use crate::PublicModelName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSettings {
    allowed: Vec<String>,
}

impl ModelSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let SettingValue::StringList(allowed) =
            overrides.effective_value(SettingKey::ModelsAllowed)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
        Ok(Self { allowed })
    }

    #[must_use]
    pub fn allowed(&self) -> &[String] {
        &self.allowed
    }

    #[must_use]
    pub fn allows(&self, model: &PublicModelName) -> bool {
        self.allowed.is_empty()
            || self
                .allowed
                .binary_search_by(|candidate| candidate.as_str().cmp(model.as_str()))
                .is_ok()
    }
}

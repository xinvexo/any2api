use std::collections::BTreeSet;

use super::{SettingKey, SettingOverrides, SettingValue, SettingsValidationError};
use crate::PublicModelName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSettings {
    allowed: BTreeSet<PublicModelName>,
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
        let allowed = allowed
            .into_iter()
            .map(PublicModelName::new)
            .collect::<Result<_, _>>()
            .map_err(|_| SettingsValidationError::InvalidListValue)?;
        Ok(Self { allowed })
    }

    #[must_use]
    pub fn allows(&self, model: &PublicModelName) -> bool {
        self.allowed.is_empty() || self.allowed.contains(model)
    }
}

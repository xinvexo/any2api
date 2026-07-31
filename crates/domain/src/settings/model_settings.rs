use std::collections::BTreeSet;

use super::{ModelAccess, SettingKey, SettingOverrides, SettingValue, SettingsValidationError};
use crate::PublicModelName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSettings {
    allowed: Option<BTreeSet<PublicModelName>>,
}

impl ModelSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let SettingValue::ModelAccess(access) =
            overrides.effective_value(SettingKey::ModelsAllowed)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
        let allowed = match access {
            ModelAccess::All => None,
            ModelAccess::Allowlist(allowed) => Some(
                allowed
                    .into_iter()
                    .map(PublicModelName::new)
                    .collect::<Result<_, _>>()
                    .map_err(|_| SettingsValidationError::InvalidListValue)?,
            ),
        };
        Ok(Self { allowed })
    }

    #[must_use]
    pub fn allows(&self, model: &PublicModelName) -> bool {
        self.allowed
            .as_ref()
            .is_none_or(|allowed| allowed.contains(model))
    }
}

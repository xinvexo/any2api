use std::collections::BTreeSet;

use super::{ModelAccess, SettingKey, SettingOverrides, SettingValue, SettingsValidationError};
use crate::PublicModelName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSettings {
    allowed: Option<BTreeSet<PublicModelName>>,
    fast_enabled: bool,
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
        let SettingValue::Boolean(fast_enabled) =
            overrides.effective_value(SettingKey::ModelsFastEnabled)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
        Ok(Self {
            allowed,
            fast_enabled,
        })
    }

    #[must_use]
    pub fn allows(&self, model: &PublicModelName) -> bool {
        self.allowed
            .as_ref()
            .is_none_or(|allowed| allowed.contains(model))
    }

    #[must_use]
    pub const fn fast_enabled(&self) -> bool {
        self.fast_enabled
    }
}

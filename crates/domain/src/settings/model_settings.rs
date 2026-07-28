use std::collections::BTreeSet;

use super::{SettingKey, SettingOverrides, SettingValue, SettingsValidationError};
use crate::PublicModelName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelAccessPolicy {
    All,
    Only(BTreeSet<PublicModelName>),
}

impl ModelAccessPolicy {
    #[must_use]
    pub fn allows(&self, model: &PublicModelName) -> bool {
        match self {
            Self::All => true,
            Self::Only(allowed) => allowed.contains(model),
        }
    }

    #[must_use]
    pub const fn only(&self) -> Option<&BTreeSet<PublicModelName>> {
        match self {
            Self::All => None,
            Self::Only(allowed) => Some(allowed),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSettings {
    access: ModelAccessPolicy,
}

impl ModelSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let SettingValue::OptionalStringList(allowed) =
            overrides.effective_value(SettingKey::ModelsAllowed)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
        let access = match allowed {
            None => ModelAccessPolicy::All,
            Some(allowed) => ModelAccessPolicy::Only(
                allowed
                    .into_iter()
                    .map(PublicModelName::new)
                    .collect::<Result<_, _>>()
                    .map_err(|_| SettingsValidationError::InvalidListValue)?,
            ),
        };
        Ok(Self { access })
    }

    #[must_use]
    pub const fn access(&self) -> &ModelAccessPolicy {
        &self.access
    }

    #[must_use]
    pub fn allows(&self, model: &PublicModelName) -> bool {
        self.access.allows(model)
    }
}

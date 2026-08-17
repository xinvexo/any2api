use super::{SettingKey, SettingOverrides, SettingsValidationError, value::integer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReliabilitySettings {
    precommit_total_budget_secs: u64,
}

impl ReliabilitySettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        Ok(Self {
            precommit_total_budget_secs: integer(
                overrides.effective_value(SettingKey::RetryPrecommitTotalBudget),
            )?,
        })
    }
    pub const fn precommit_total_budget_secs(&self) -> u64 {
        self.precommit_total_budget_secs
    }
}

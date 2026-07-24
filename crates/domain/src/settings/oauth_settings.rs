use super::{SettingKey, SettingOverrides, SettingsValidationError, value::integer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthSettings {
    refresh_scan_interval_secs: u64,
    refresh_lead_time_secs: u64,
}

impl OAuthSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let refresh_scan_interval_secs =
            integer(overrides.effective_value(SettingKey::OAuthRefreshScanInterval))?;
        let refresh_lead_time_secs =
            integer(overrides.effective_value(SettingKey::OAuthRefreshLeadTime))?;
        if refresh_lead_time_secs < refresh_scan_interval_secs {
            return Err(SettingsValidationError::InvalidCombination);
        }
        Ok(Self {
            refresh_scan_interval_secs,
            refresh_lead_time_secs,
        })
    }

    pub const fn refresh_scan_interval_secs(&self) -> u64 {
        self.refresh_scan_interval_secs
    }

    pub const fn refresh_lead_time_secs(&self) -> u64 {
        self.refresh_lead_time_secs
    }
}

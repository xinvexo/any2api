use super::{
    CodexQuotaRateCard, SettingKey, SettingOverrides, SettingValue, SettingsValidationError,
    value::integer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthSettings {
    refresh_scan_interval_secs: u64,
    refresh_lead_time_secs: u64,
    codex_rate_card: CodexQuotaRateCard,
}

impl OAuthSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let refresh_scan_interval_secs =
            integer(overrides.effective_value(SettingKey::OAuthRefreshScanInterval))?;
        let refresh_lead_time_secs =
            integer(overrides.effective_value(SettingKey::OAuthRefreshLeadTime))?;
        let codex_rate_card = match overrides.effective_value(SettingKey::OAuthCodexRateCard) {
            SettingValue::CodexRateCard(value) => value,
            _ => return Err(SettingsValidationError::InvalidType),
        };
        if refresh_lead_time_secs < refresh_scan_interval_secs {
            return Err(SettingsValidationError::InvalidCombination);
        }
        Ok(Self {
            refresh_scan_interval_secs,
            refresh_lead_time_secs,
            codex_rate_card,
        })
    }

    pub const fn refresh_scan_interval_secs(&self) -> u64 {
        self.refresh_scan_interval_secs
    }

    pub const fn refresh_lead_time_secs(&self) -> u64 {
        self.refresh_lead_time_secs
    }

    pub const fn codex_rate_card(&self) -> &CodexQuotaRateCard {
        &self.codex_rate_card
    }
}

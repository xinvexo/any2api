use ipnet::IpNet;

use super::{SettingKey, SettingOverrides, SettingValue, SettingsValidationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSettings {
    trusted_proxy_cidrs: Vec<IpNet>,
}

impl NetworkSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let SettingValue::StringList(values) =
            overrides.effective_value(SettingKey::NetworkTrustedProxyCidrs)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
        let trusted_proxy_cidrs = values
            .into_iter()
            .map(|value| {
                value
                    .parse::<IpNet>()
                    .map_err(|_| SettingsValidationError::InvalidListValue)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            trusted_proxy_cidrs,
        })
    }

    pub fn trusted_proxy_cidrs(&self) -> &[IpNet] {
        &self.trusted_proxy_cidrs
    }
}

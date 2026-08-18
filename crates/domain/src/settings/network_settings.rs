use std::num::NonZeroUsize;

use ipnet::IpNet;

use super::{SettingKey, SettingOverrides, SettingValue, SettingsValidationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSettings {
    max_connections: NonZeroUsize,
    trusted_proxy_cidrs: Vec<IpNet>,
}

impl NetworkSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let SettingValue::Integer(max_connections) =
            overrides.effective_value(SettingKey::NetworkMaxConnections)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
        let max_connections = usize::try_from(max_connections)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(SettingsValidationError::OutOfRange)?;
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
            max_connections,
            trusted_proxy_cidrs,
        })
    }

    pub const fn max_connections(&self) -> NonZeroUsize {
        self.max_connections
    }

    pub fn trusted_proxy_cidrs(&self) -> &[IpNet] {
        &self.trusted_proxy_cidrs
    }
}

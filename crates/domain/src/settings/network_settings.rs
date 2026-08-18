use std::num::NonZeroUsize;

use ipnet::IpNet;

use super::{SettingKey, SettingOverrides, SettingValue, SettingsValidationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSettings {
    max_connections: NonZeroUsize,
    request_header_timeout_secs: u64,
    request_body_idle_timeout_secs: u64,
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
        let SettingValue::DurationSecs(request_header_timeout_secs) =
            overrides.effective_value(SettingKey::NetworkRequestHeaderTimeout)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
        let SettingValue::DurationSecs(request_body_idle_timeout_secs) =
            overrides.effective_value(SettingKey::NetworkRequestBodyIdleTimeout)
        else {
            return Err(SettingsValidationError::InvalidType);
        };
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
            request_header_timeout_secs,
            request_body_idle_timeout_secs,
            trusted_proxy_cidrs,
        })
    }

    pub const fn max_connections(&self) -> NonZeroUsize {
        self.max_connections
    }

    pub const fn request_header_timeout_secs(&self) -> u64 {
        self.request_header_timeout_secs
    }

    pub const fn request_body_idle_timeout_secs(&self) -> u64 {
        self.request_body_idle_timeout_secs
    }

    pub fn trusted_proxy_cidrs(&self) -> &[IpNet] {
        &self.trusted_proxy_cidrs
    }
}

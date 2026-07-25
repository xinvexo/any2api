use serde::{Deserialize, Deserializer, Serialize, de};
use url::Host;

use crate::proxy::ProxyValidationError;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProxyAddress {
    host: String,
    port: u16,
}

impl ProxyAddress {
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, ProxyValidationError> {
        let host = normalize_host(host.into())?;
        if port == 0 {
            return Err(ProxyValidationError::InvalidPort);
        }

        Ok(Self { host, port })
    }

    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }
}

impl<'de> Deserialize<'de> for ProxyAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = ProxyAddressInput::deserialize(deserializer)?;
        Self::new(input.host, input.port).map_err(de::Error::custom)
    }
}

#[derive(Deserialize)]
struct ProxyAddressInput {
    host: String,
    port: u16,
}

fn normalize_host(host: String) -> Result<String, ProxyValidationError> {
    if host.is_empty() || host.trim() != host {
        return Err(ProxyValidationError::InvalidHost);
    }

    Host::parse(&host)
        .map(|parsed| parsed.to_string())
        .map_err(|_| ProxyValidationError::InvalidHost)
}

#[cfg(test)]
mod tests {
    use super::ProxyAddress;

    #[test]
    fn deserialization_reuses_address_validation() {
        let error = serde_json::from_str::<ProxyAddress>(r#"{"host":"","port":0}"#)
            .expect_err("invalid serialized address must fail");

        assert!(error.to_string().contains("proxy host is invalid"));
    }
}

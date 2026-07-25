use std::fmt;

use secrecy::{ExposeSecret, SecretString};

#[derive(Clone)]
pub struct ProviderSecret {
    schema_version: u32,
    value: SecretString,
}

impl ProviderSecret {
    #[must_use]
    pub fn new(schema_version: u32, value: impl Into<String>) -> Self {
        Self {
            schema_version,
            value: SecretString::from(value.into()),
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        self.value.expose_secret()
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderSecret")
            .field("schema_version", &self.schema_version)
            .field("value", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderSecret;

    #[test]
    fn debug_output_never_contains_secret() {
        let secret = ProviderSecret::new(1, "very-secret-value");
        let debug = format!("{secret:?}");

        assert!(!debug.contains("very-secret-value"));
        assert!(debug.contains("REDACTED"));
    }
}

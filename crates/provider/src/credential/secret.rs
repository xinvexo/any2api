use std::fmt;

use secrecy::{ExposeSecret, SecretString};

#[derive(Clone)]
pub struct ProviderSecret {
    value: SecretString,
}

impl ProviderSecret {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: SecretString::from(value.into()),
        }
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
            .field("value", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderSecret;

    #[test]
    fn debug_output_never_contains_secret() {
        let secret = ProviderSecret::new("very-secret-value");
        let debug = format!("{secret:?}");

        assert!(!debug.contains("very-secret-value"));
        assert!(debug.contains("REDACTED"));
    }
}

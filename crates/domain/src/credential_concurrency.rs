use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_CREDENTIAL_CONCURRENCY: u32 = 10_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct MaxConcurrency(u32);

impl MaxConcurrency {
    pub fn new(value: u32) -> Result<Self, MaxConcurrencyError> {
        if (1..=MAX_CREDENTIAL_CONCURRENCY).contains(&value) {
            Ok(Self(value))
        } else {
            Err(MaxConcurrencyError)
        }
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("max concurrency must be between 1 and {MAX_CREDENTIAL_CONCURRENCY}")]
pub struct MaxConcurrencyError;

#[cfg(test)]
mod tests {
    use super::{MAX_CREDENTIAL_CONCURRENCY, MaxConcurrency};

    #[test]
    fn concurrency_limit_has_strict_bounds() {
        assert!(MaxConcurrency::new(0).is_err());
        assert!(MaxConcurrency::new(1).is_ok());
        assert!(MaxConcurrency::new(MAX_CREDENTIAL_CONCURRENCY).is_ok());
        assert!(MaxConcurrency::new(MAX_CREDENTIAL_CONCURRENCY + 1).is_err());
    }
}

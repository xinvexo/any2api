use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ConfigRevision(u64);

impl ConfigRevision {
    pub const INITIAL: Self = Self(1);

    pub const fn new(value: u64) -> Result<Self, ConfigRevisionError> {
        if value == 0 {
            return Err(ConfigRevisionError::Zero);
        }

        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, ConfigRevisionError> {
        self.0
            .checked_add(1)
            .ok_or(ConfigRevisionError::Overflow)
            .and_then(Self::new)
    }
}

impl<'de> Deserialize<'de> for ConfigRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ConfigRevisionError {
    #[error("configuration revision must be greater than zero")]
    Zero,
    #[error("configuration revision overflow")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::ConfigRevision;

    #[test]
    fn revision_increments_monotonically() {
        let next = ConfigRevision::INITIAL
            .checked_next()
            .expect("next revision");

        assert_eq!(next.get(), 2);
    }

    #[test]
    fn zero_revision_is_rejected() {
        let error = ConfigRevision::new(0).expect_err("zero must be invalid");

        assert_eq!(error, super::ConfigRevisionError::Zero);
    }
}

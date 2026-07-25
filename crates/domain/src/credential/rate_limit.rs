use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_REQUESTS_PER_MINUTE: u32 = 100_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RequestsPerMinute(u32);

impl RequestsPerMinute {
    pub fn new(value: u32) -> Result<Self, RequestsPerMinuteError> {
        if (1..=MAX_REQUESTS_PER_MINUTE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(RequestsPerMinuteError)
        }
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("requests per minute must be between 1 and {MAX_REQUESTS_PER_MINUTE}")]
pub struct RequestsPerMinuteError;

#[cfg(test)]
mod tests {
    use super::{MAX_REQUESTS_PER_MINUTE, RequestsPerMinute};

    #[test]
    fn request_rate_has_strict_bounds() {
        assert!(RequestsPerMinute::new(0).is_err());
        assert!(RequestsPerMinute::new(1).is_ok());
        assert!(RequestsPerMinute::new(MAX_REQUESTS_PER_MINUTE).is_ok());
        assert!(RequestsPerMinute::new(MAX_REQUESTS_PER_MINUTE + 1).is_err());
    }
}

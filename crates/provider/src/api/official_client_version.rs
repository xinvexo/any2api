use std::{fmt, str::FromStr};

use http::{HeaderMap, Method};
use semver::Version;
use thiserror::Error;
use url::Url;

/// A validated stable semantic version used by an official upstream client.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OfficialClientVersion(String);

impl OfficialClientVersion {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidOfficialClientVersion> {
        let value = value.into();
        let parsed = Version::parse(&value).map_err(|_| InvalidOfficialClientVersion)?;
        if !parsed.pre.is_empty() || parsed.to_string() != value {
            return Err(InvalidOfficialClientVersion);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for OfficialClientVersion {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for OfficialClientVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for OfficialClientVersion {
    type Err = InvalidOfficialClientVersion;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("official client version must be a stable semantic version")]
pub struct InvalidOfficialClientVersion;

#[derive(Clone)]
pub struct OfficialClientVersionRequestPlan {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl fmt::Debug for OfficialClientVersionRequestPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfficialClientVersionRequestPlan")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("header_count", &self.headers.len())
            .field("body_len", &self.body.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::OfficialClientVersion;

    #[test]
    fn accepts_only_canonical_stable_semver() {
        for version in ["0.149.0", "2.1.240", "1.0.5+official.1"] {
            assert_eq!(
                OfficialClientVersion::new(version)
                    .expect("stable version")
                    .as_str(),
                version
            );
        }
        for version in ["rust-v0.149.0", "1.0", "1.0.0-beta.1", " 1.0.0"] {
            assert!(OfficialClientVersion::new(version).is_err(), "{version}");
        }
    }
}

use std::fmt;

use thiserror::Error;

pub const CREDENTIAL_FINGERPRINT_VERSION: u16 = 1;
pub const CREDENTIAL_FINGERPRINT_LENGTH: usize = 32;

#[derive(Clone, Eq, PartialEq)]
pub struct CredentialSecretFingerprint {
    version: u16,
    digest: [u8; CREDENTIAL_FINGERPRINT_LENGTH],
    tail: Option<String>,
}

impl CredentialSecretFingerprint {
    pub fn new(
        digest: [u8; CREDENTIAL_FINGERPRINT_LENGTH],
        tail: Option<String>,
    ) -> Result<Self, CredentialFingerprintError> {
        Self::restore(CREDENTIAL_FINGERPRINT_VERSION, digest, tail)
    }

    pub fn restore(
        version: u16,
        digest: [u8; CREDENTIAL_FINGERPRINT_LENGTH],
        tail: Option<String>,
    ) -> Result<Self, CredentialFingerprintError> {
        if version != CREDENTIAL_FINGERPRINT_VERSION {
            return Err(CredentialFingerprintError::UnsupportedVersion);
        }
        if tail.as_deref().is_some_and(|value| {
            value.len() != 4 || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        }) {
            return Err(CredentialFingerprintError::InvalidTail);
        }
        Ok(Self {
            version,
            digest,
            tail,
        })
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub const fn digest(&self) -> &[u8; CREDENTIAL_FINGERPRINT_LENGTH] {
        &self.digest
    }

    #[must_use]
    pub fn tail(&self) -> Option<&str> {
        self.tail.as_deref()
    }

    #[must_use]
    pub fn display(&self) -> String {
        let mut value = format!("v{}:", self.version);
        for byte in &self.digest[..8] {
            use fmt::Write as _;
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
        }
        value
    }
}

impl fmt::Debug for CredentialSecretFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialSecretFingerprint")
            .field("display", &self.display())
            .field("tail", &self.tail)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CredentialFingerprintError {
    #[error("credential fingerprint version is unsupported")]
    UnsupportedVersion,
    #[error("credential secret tail is invalid")]
    InvalidTail,
}

#[cfg(test)]
mod tests {
    use super::CredentialSecretFingerprint;

    #[test]
    fn display_is_versioned_and_truncated() {
        let fingerprint = CredentialSecretFingerprint::new([0xab; 32], Some("test".to_owned()))
            .expect("fingerprint");

        assert_eq!(fingerprint.display(), "v1:abababababababab");
        assert_eq!(fingerprint.tail(), Some("test"));
        assert!(!format!("{fingerprint:?}").contains(&"ab".repeat(32)));
    }
}

use std::fmt;

use super::{context::AAD_VERSION, error::SecretVaultError};

pub(crate) const ENVELOPE_VERSION: u16 = 1;
pub(crate) const NONCE_LENGTH: usize = 24;
const MIN_CIPHERTEXT_LENGTH: usize = 16;
const ALGORITHM_NAME: &str = "xchacha20poly1305";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretAlgorithm {
    XChaCha20Poly1305,
}

impl SecretAlgorithm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::XChaCha20Poly1305 => ALGORITHM_NAME,
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, SecretVaultError> {
        match value {
            ALGORITHM_NAME => Ok(Self::XChaCha20Poly1305),
            _ => Err(SecretVaultError::UnsupportedEnvelopeAlgorithm),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SecretEnvelope {
    version: u16,
    key_id: String,
    algorithm: SecretAlgorithm,
    nonce: [u8; NONCE_LENGTH],
    ciphertext: Vec<u8>,
    aad_version: u16,
}

impl SecretEnvelope {
    pub fn restore(
        version: u16,
        key_id: impl Into<String>,
        algorithm: &str,
        nonce: &[u8],
        ciphertext: Vec<u8>,
        aad_version: u16,
    ) -> Result<Self, SecretVaultError> {
        if version != ENVELOPE_VERSION {
            return Err(SecretVaultError::UnsupportedEnvelopeVersion);
        }
        if aad_version != AAD_VERSION {
            return Err(SecretVaultError::UnsupportedAadVersion);
        }
        let key_id = key_id.into();
        if key_id.is_empty() || key_id.len() > 128 {
            return Err(SecretVaultError::InvalidEnvelope);
        }
        let nonce = nonce
            .try_into()
            .map_err(|_| SecretVaultError::InvalidEnvelope)?;
        if ciphertext.len() < MIN_CIPHERTEXT_LENGTH {
            return Err(SecretVaultError::InvalidEnvelope);
        }
        Ok(Self {
            version,
            key_id,
            algorithm: SecretAlgorithm::parse(algorithm)?,
            nonce,
            ciphertext,
            aad_version,
        })
    }

    pub(crate) fn new(key_id: String, nonce: [u8; NONCE_LENGTH], ciphertext: Vec<u8>) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            key_id,
            algorithm: SecretAlgorithm::XChaCha20Poly1305,
            nonce,
            ciphertext,
            aad_version: AAD_VERSION,
        }
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub const fn algorithm(&self) -> SecretAlgorithm {
        self.algorithm
    }

    #[must_use]
    pub const fn nonce(&self) -> &[u8; NONCE_LENGTH] {
        &self.nonce
    }

    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    #[must_use]
    pub const fn aad_version(&self) -> u16 {
        self.aad_version
    }
}

impl fmt::Debug for SecretEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretEnvelope")
            .field("version", &self.version)
            .field("key_id", &self.key_id)
            .field("algorithm", &self.algorithm)
            .field("nonce", &"[REDACTED]")
            .field("ciphertext_len", &self.ciphertext.len())
            .field("aad_version", &self.aad_version)
            .finish()
    }
}

use any2api_domain::{CredentialKind, CredentialSecretFingerprint, ProviderKind};
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::secret::SecretBytes;

const MAX_API_KEY_BYTES: usize = 8_192;
const FINGERPRINT_DOMAIN: &[u8] = b"any2api-provider-credential-fingerprint-v2";

pub(crate) fn build_fingerprint(
    provider_kind: ProviderKind,
    credential_kind: CredentialKind,
    secret: &SecretBytes,
) -> Result<CredentialSecretFingerprint, ProviderApiKeyValidationError> {
    let value = secret.expose_secret();
    validate(value)?;
    let tail = (value.len() >= 8).then(|| {
        String::from_utf8(value[value.len() - 4..].to_vec())
            .expect("validated API Key bytes are ASCII")
    });
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    digest.update([provider_kind_code(provider_kind)]);
    digest.update([credential_kind_code(credential_kind)]);
    digest.update(value);
    CredentialSecretFingerprint::new(digest.finalize().into(), tail)
        .map_err(|_| ProviderApiKeyValidationError::InvalidCharacters)
}

const fn provider_kind_code(kind: ProviderKind) -> u8 {
    match kind {
        ProviderKind::Codex => 1,
        ProviderKind::Claude => 2,
        ProviderKind::Grok => 3,
    }
}

const fn credential_kind_code(kind: CredentialKind) -> u8 {
    match kind {
        CredentialKind::ApiKey => 1,
    }
}

pub(crate) fn validate(value: &[u8]) -> Result<(), ProviderApiKeyValidationError> {
    if value.is_empty() {
        return Err(ProviderApiKeyValidationError::Empty);
    }
    if value.len() > MAX_API_KEY_BYTES {
        return Err(ProviderApiKeyValidationError::TooLong);
    }
    if !value.iter().all(|byte| (0x21..=0x7e).contains(byte)) {
        return Err(ProviderApiKeyValidationError::InvalidCharacters);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProviderApiKeyValidationError {
    #[error("provider API Key must not be empty")]
    Empty,
    #[error("provider API Key is too long")]
    TooLong,
    #[error("provider API Key must contain only visible ASCII characters")]
    InvalidCharacters,
}

#[cfg(test)]
mod tests {
    use any2api_domain::{CredentialKind, ProviderKind};

    use super::{ProviderApiKeyValidationError, build_fingerprint, validate};

    #[test]
    fn api_key_rejects_whitespace_and_control_characters() {
        assert_eq!(validate(b""), Err(ProviderApiKeyValidationError::Empty));
        assert_eq!(
            validate(b"key with spaces"),
            Err(ProviderApiKeyValidationError::InvalidCharacters)
        );
        assert!(validate(b"sk-valid_123").is_ok());
    }

    #[test]
    fn fingerprint_is_stable_and_domain_separated() {
        let fingerprint = build_fingerprint(
            ProviderKind::Codex,
            CredentialKind::ApiKey,
            &b"sk-valid_123".to_vec().into(),
        )
        .expect("fingerprint");
        assert_eq!(
            fingerprint.digest(),
            &[
                0xbb, 0xe9, 0x25, 0x90, 0xe2, 0x88, 0x9a, 0x17, 0xd0, 0x56, 0xb2, 0x82, 0x90, 0x7e,
                0x60, 0xe5, 0x32, 0xcd, 0x16, 0x7c, 0x02, 0x18, 0xb3, 0xfd, 0x93, 0xed, 0x0f, 0xf8,
                0x81, 0x7e, 0xe8, 0x90,
            ]
        );
    }
}

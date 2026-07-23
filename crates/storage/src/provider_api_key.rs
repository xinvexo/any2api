use any2api_domain::{CredentialKind, CredentialSecretFingerprint, ProviderKind};
use secrecy::ExposeSecret;
use thiserror::Error;

use crate::vault::{SecretBytes, SecretVault};

const MAX_API_KEY_BYTES: usize = 8_192;
const MAX_OAUTH2_SECRET_BYTES: usize = 65_536;

pub(crate) fn build_fingerprint(
    vault: &SecretVault,
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
    CredentialSecretFingerprint::new(
        vault.credential_fingerprint(provider_kind, credential_kind, secret),
        tail,
    )
    .map_err(|_| ProviderApiKeyValidationError::InvalidCharacters)
}

pub(crate) fn build_oauth2_fingerprint(
    vault: &SecretVault,
    provider_kind: ProviderKind,
    secret: &SecretBytes,
) -> Result<CredentialSecretFingerprint, ProviderOAuth2SecretValidationError> {
    let value = secret.expose_secret();
    validate_oauth2(value)?;
    CredentialSecretFingerprint::new(
        vault.credential_fingerprint(provider_kind, CredentialKind::OAuth2, secret),
        None,
    )
    .map_err(|_| ProviderOAuth2SecretValidationError::InvalidEncoding)
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

pub(crate) fn validate_oauth2(value: &[u8]) -> Result<(), ProviderOAuth2SecretValidationError> {
    if value.is_empty() {
        return Err(ProviderOAuth2SecretValidationError::Empty);
    }
    if value.len() > MAX_OAUTH2_SECRET_BYTES {
        return Err(ProviderOAuth2SecretValidationError::TooLong);
    }
    std::str::from_utf8(value)
        .map(|_| ())
        .map_err(|_| ProviderOAuth2SecretValidationError::InvalidEncoding)
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

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProviderOAuth2SecretValidationError {
    #[error("provider OAuth2 secret must not be empty")]
    Empty,
    #[error("provider OAuth2 secret is too long")]
    TooLong,
    #[error("provider OAuth2 secret must be valid UTF-8")]
    InvalidEncoding,
}

#[cfg(test)]
mod tests {
    use super::{ProviderApiKeyValidationError, validate};

    #[test]
    fn api_key_rejects_whitespace_and_control_characters() {
        assert_eq!(validate(b""), Err(ProviderApiKeyValidationError::Empty));
        assert_eq!(
            validate(b"key with spaces"),
            Err(ProviderApiKeyValidationError::InvalidCharacters)
        );
        assert!(validate(b"sk-valid_123").is_ok());
    }
}

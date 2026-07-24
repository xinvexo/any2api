use std::fmt;

use any2api_domain::ProviderKind;
use secrecy::ExposeSecret;
use serde_json::Value;
use thiserror::Error;

use crate::vault::SecretBytes;

pub const MAX_OAUTH_ACCOUNT_JSON_BYTES: usize = 64 * 1024;

pub struct OAuthAccountDocument {
    provider_kind: ProviderKind,
    bytes: SecretBytes,
}

impl OAuthAccountDocument {
    pub fn new(
        provider: ProviderKind,
        bytes: SecretBytes,
    ) -> Result<Self, OAuthAccountDocumentValidationError> {
        validate(provider, bytes.expose_secret())?;
        Ok(Self {
            provider_kind: provider,
            bytes,
        })
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub fn into_bytes(self) -> SecretBytes {
        self.bytes
    }

    pub(crate) fn expose(&self) -> &[u8] {
        self.bytes.expose_secret()
    }

    #[cfg(test)]
    pub(crate) fn expose_for_test(&self) -> &[u8] {
        self.expose()
    }
}

impl fmt::Debug for OAuthAccountDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthAccountDocument")
            .field("provider_kind", &self.provider_kind)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum OAuthAccountDocumentValidationError {
    #[error("OAuth account JSON is empty")]
    Empty,
    #[error("OAuth account JSON is too large")]
    TooLarge,
    #[error("OAuth account JSON is invalid")]
    InvalidJson,
    #[error("OAuth account JSON provider does not match")]
    ProviderMismatch,
    #[error("OAuth account JSON does not contain an access token")]
    MissingAccessToken,
}

fn validate(
    provider: ProviderKind,
    bytes: &[u8],
) -> Result<(), OAuthAccountDocumentValidationError> {
    if bytes.is_empty() {
        return Err(OAuthAccountDocumentValidationError::Empty);
    }
    if bytes.len() > MAX_OAUTH_ACCOUNT_JSON_BYTES {
        return Err(OAuthAccountDocumentValidationError::TooLarge);
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| OAuthAccountDocumentValidationError::InvalidJson)?;
    let object = value
        .as_object()
        .ok_or(OAuthAccountDocumentValidationError::InvalidJson)?;
    let expected_provider = match provider {
        ProviderKind::Codex => "codex",
        ProviderKind::Claude => "claude",
    };
    if object.get("type").and_then(Value::as_str) != Some(expected_provider) {
        return Err(OAuthAccountDocumentValidationError::ProviderMismatch);
    }
    let access_token = object
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or(OAuthAccountDocumentValidationError::MissingAccessToken)?;
    if access_token.trim().is_empty() {
        return Err(OAuthAccountDocumentValidationError::MissingAccessToken);
    }
    Ok(())
}

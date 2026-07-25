use std::fmt;

use any2api_domain::{
    GATEWAY_TOKEN_BODY_LEN, GATEWAY_TOKEN_PREFIX, GatewayApiKeyValidationError,
    validate_gateway_token,
};
use any2api_storage::api::SecretBytes;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

const TOKEN_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

#[derive(Debug, Error)]
pub enum GatewayApiKeyTokenError {
    #[error("failed to generate a gateway API Key token")]
    Generation,
    #[error(transparent)]
    Invalid(#[from] GatewayApiKeyValidationError),
}

/// Back-compat alias used by publish error mapping.
pub type GatewayApiKeyTokenGenerationError = GatewayApiKeyTokenError;

pub struct GatewayApiKeyToken(SecretString);

impl GatewayApiKeyToken {
    pub fn generate() -> Result<Self, GatewayApiKeyTokenError> {
        let mut body = String::with_capacity(GATEWAY_TOKEN_BODY_LEN);
        // Rejection sampling keeps the A-Za-z0-9 distribution unbiased.
        while body.len() < GATEWAY_TOKEN_BODY_LEN {
            let mut byte = [0_u8; 1];
            getrandom::fill(&mut byte).map_err(|_| GatewayApiKeyTokenError::Generation)?;
            let value = byte[0];
            if value < 248 {
                body.push(TOKEN_ALPHABET[(value % 62) as usize] as char);
            }
        }
        Self::parse(format!("{GATEWAY_TOKEN_PREFIX}{body}"))
    }

    pub fn parse(token: impl Into<String>) -> Result<Self, GatewayApiKeyTokenError> {
        let token = validate_gateway_token(token.into())?;
        Ok(Self(SecretString::from(token)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.expose_secret()
    }

    pub(crate) fn storage_secret(&self) -> SecretBytes {
        self.0.expose_secret().as_bytes().to_vec().into()
    }
}

impl fmt::Debug for GatewayApiKeyToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayApiKeyToken")
            .field("token", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::GatewayApiKeyToken;
    use any2api_domain::{GATEWAY_TOKEN_BODY_LEN, GATEWAY_TOKEN_PREFIX};

    #[test]
    fn generate_uses_sk_prefix_and_alphanumeric_body() {
        let token = GatewayApiKeyToken::generate().expect("generate");
        let value = token.as_str();
        assert!(value.starts_with(GATEWAY_TOKEN_PREFIX));
        let body = value.strip_prefix(GATEWAY_TOKEN_PREFIX).expect("prefix");
        assert_eq!(body.len(), GATEWAY_TOKEN_BODY_LEN);
        assert!(body.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    }

    #[test]
    fn parse_rejects_invalid_shapes() {
        assert!(GatewayApiKeyToken::parse("sk-short").is_err());
        assert!(GatewayApiKeyToken::parse(format!("a2k_v1_{}", "a".repeat(43))).is_err());
    }
}

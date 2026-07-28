use std::fmt;

use any2api_domain::GATEWAY_TOKEN_PREFIX;
use any2api_storage::api::SecretBytes;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum GatewayApiKeyTokenError {
    #[error("failed to generate a gateway API Key token")]
    Generation,
}

pub(crate) struct GatewayApiKeyToken(SecretString);

impl GatewayApiKeyToken {
    pub(crate) fn generate() -> Result<Self, GatewayApiKeyTokenError> {
        let mut random = [0_u8; 32];
        getrandom::fill(&mut random).map_err(|_| GatewayApiKeyTokenError::Generation)?;
        Ok(Self(SecretString::from(format!(
            "{GATEWAY_TOKEN_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(random)
        ))))
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
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
    use any2api_domain::{GATEWAY_TOKEN_BODY_LEN, GATEWAY_TOKEN_PREFIX, validate_gateway_token};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    #[test]
    fn generate_uses_a_versioned_32_byte_urlsafe_body() {
        let token = GatewayApiKeyToken::generate().expect("generate");
        let value = token.as_str();
        assert!(value.starts_with(GATEWAY_TOKEN_PREFIX));
        let body = value.strip_prefix(GATEWAY_TOKEN_PREFIX).expect("prefix");
        assert_eq!(body.len(), GATEWAY_TOKEN_BODY_LEN);
        assert_eq!(URL_SAFE_NO_PAD.decode(body).expect("base64 body").len(), 32);
        assert!(validate_gateway_token(value.to_owned()).is_ok());
    }
}

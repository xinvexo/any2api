use std::fmt;

use any2api_domain::{GATEWAY_TOKEN_PREFIX, GATEWAY_TOKEN_RANDOM_BYTES};
use any2api_storage::api::SecretBytes;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("failed to generate a gateway API Key token")]
pub struct GatewayApiKeyTokenGenerationError;

pub struct GatewayApiKeyToken(SecretString);

impl GatewayApiKeyToken {
    pub fn generate() -> Result<Self, GatewayApiKeyTokenGenerationError> {
        let mut random = [0_u8; GATEWAY_TOKEN_RANDOM_BYTES];
        getrandom::fill(&mut random).map_err(|_| GatewayApiKeyTokenGenerationError)?;
        Ok(Self(SecretString::from(format!(
            "{GATEWAY_TOKEN_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(random)
        ))))
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

use any2api_provider::api::{OAuthTokenMaterial, ProviderError};
use any2api_storage::api::SecretBytes;
use secrecy::{ExposeSecret, SecretString};

#[derive(Clone)]
pub struct ProviderOAuth2Secret(SecretString);

impl ProviderOAuth2Secret {
    pub(crate) fn from_token(token: &OAuthTokenMaterial) -> Result<Self, ProviderError> {
        let secret = token.to_secret()?;
        Ok(Self(SecretString::from(secret.expose().to_owned())))
    }

    pub(crate) fn into_storage_secret(self) -> SecretBytes {
        self.0.expose_secret().as_bytes().to_vec().into()
    }
}

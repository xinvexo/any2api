use any2api_storage::api::SecretBytes;
use secrecy::{ExposeSecret, SecretString};

pub struct ProviderApiKeySecret(SecretString);

impl ProviderApiKeySecret {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value.into())
    }

    pub(crate) fn into_storage_secret(self) -> SecretBytes {
        self.0.expose_secret().as_bytes().to_vec().into()
    }
}

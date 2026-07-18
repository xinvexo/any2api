use std::fmt;

use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, SecretBox, zeroize::Zeroizing};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

const TOKEN_HASH_DOMAIN: &[u8] = b"any2api-gateway-api-key-token-hash-v1";

pub struct GatewayApiKeyVerifier {
    key: SecretBox<[u8; 32]>,
    key_id: String,
}

impl GatewayApiKeyVerifier {
    pub(crate) fn from_master_key(master_key: &[u8; 32], master_key_id: &str) -> Self {
        let mut mac =
            <HmacSha256 as Mac>::new_from_slice(master_key).expect("HMAC accepts any key length");
        mac.update(b"any2api-gateway-api-key-hmac-key-v1");
        let derived = mac.finalize().into_bytes();
        let mut key_material = Zeroizing::new([0_u8; 32]);
        key_material.copy_from_slice(&derived);
        let key = SecretBox::init_with_mut(|value: &mut [u8; 32]| {
            value.copy_from_slice(key_material.as_ref())
        });
        Self {
            key,
            key_id: format!("gk1_{master_key_id}"),
        }
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    #[must_use]
    pub fn hash(&self, token: &[u8]) -> [u8; 32] {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(self.key.expose_secret())
            .expect("HMAC accepts any key length");
        mac.update(TOKEN_HASH_DOMAIN);
        mac.update(token);
        mac.finalize().into_bytes().into()
    }

    #[must_use]
    pub fn verify(&self, token: &[u8], expected: &[u8; 32]) -> bool {
        bool::from(self.hash(token).ct_eq(expected))
    }
}

impl fmt::Debug for GatewayApiKeyVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayApiKeyVerifier")
            .field("key_id", &self.key_id)
            .field("key", &"[REDACTED]")
            .finish()
    }
}

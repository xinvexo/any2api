use std::fmt;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const TOKEN_HASH_DOMAIN: &[u8] = b"any2api-gateway-api-key-token-hash-v2";

#[derive(Clone, Copy, Default)]
pub struct GatewayApiKeyVerifier;

impl GatewayApiKeyVerifier {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn hash(&self, token: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(TOKEN_HASH_DOMAIN);
        digest.update(token);
        digest.finalize().into()
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
            .field("algorithm", &"sha256")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::GatewayApiKeyVerifier;

    #[test]
    fn digest_is_stable_domain_separated_and_unkeyed() {
        let verifier = GatewayApiKeyVerifier::new();
        assert_eq!(
            verifier.hash(b"token"),
            [
                0xbf, 0xf6, 0x2f, 0x42, 0xac, 0xa1, 0xfa, 0x4e, 0xc0, 0xf3, 0xac, 0x9d, 0xde, 0x31,
                0x16, 0x72, 0x35, 0x97, 0x69, 0xf1, 0x3a, 0x8c, 0x68, 0x44, 0x62, 0x41, 0xeb, 0xcf,
                0xd5, 0x0a, 0x0d, 0xfe,
            ]
        );
        assert!(verifier.verify(b"token", &verifier.hash(b"token")));
        assert!(!verifier.verify(b"other", &verifier.hash(b"token")));
    }
}

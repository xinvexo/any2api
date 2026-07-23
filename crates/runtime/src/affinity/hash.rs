use std::{fmt, hash::Hash};

use any2api_domain::{ModelRouteId, ProtocolDialect};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(super) struct SessionHash([u8; 32]);

impl SessionHash {
    pub(super) fn prefix(self) -> String {
        URL_SAFE_NO_PAD.encode(&self.0[..9])
    }
}

impl fmt::Debug for SessionHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SessionHash")
            .field(&self.prefix())
            .finish()
    }
}

pub(super) struct SessionHasher {
    key: [u8; 32],
}

impl SessionHasher {
    pub(super) fn new() -> Self {
        let mut key = [0_u8; 32];
        getrandom::fill(&mut key).expect("operating system randomness is required");
        Self { key }
    }

    pub(super) fn hard(&self, raw: &str) -> SessionHash {
        self.digest(b"hard\0", &[], raw.as_bytes())
    }

    pub(super) fn soft(
        &self,
        dialect: ProtocolDialect,
        route_id: ModelRouteId,
        raw: &str,
    ) -> SessionHash {
        let dialect = match dialect {
            ProtocolDialect::OpenAiResponses => [1_u8],
            ProtocolDialect::CodexBackend => [2_u8],
            ProtocolDialect::AnthropicMessages => [3_u8],
            ProtocolDialect::OpenAiChatCompletions => [4_u8],
        };
        self.digest(
            b"soft\0",
            &[&dialect, route_id.as_uuid().as_bytes()],
            raw.as_bytes(),
        )
    }

    fn digest(&self, domain: &[u8], scope: &[&[u8]], raw: &[u8]) -> SessionHash {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC accepts 256-bit key");
        mac.update(b"any2api-affinity-v1\0");
        mac.update(domain);
        for value in scope {
            mac.update(value);
            mac.update(&[0]);
        }
        mac.update(raw);
        SessionHash(mac.finalize().into_bytes().into())
    }
}

impl fmt::Debug for SessionHasher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionHasher([REDACTED])")
    }
}

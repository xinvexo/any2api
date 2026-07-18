use any2api_domain::{GATEWAY_TOKEN_PREFIX, GATEWAY_TOKEN_RANDOM_BYTES};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::ExposeSecret;

use crate::{error::StorageError, vault::SecretBytes};

const DISPLAY_PREFIX_BYTES: usize = 16;

pub(crate) fn display_prefix(token: &SecretBytes) -> Result<String, StorageError> {
    let value = token.expose_secret();
    validate(value)?;
    String::from_utf8(value[..DISPLAY_PREFIX_BYTES].to_vec())
        .map_err(|_| StorageError::InvalidGatewayApiKeyToken)
}

fn validate(value: &[u8]) -> Result<(), StorageError> {
    let suffix = value
        .strip_prefix(GATEWAY_TOKEN_PREFIX.as_bytes())
        .ok_or(StorageError::InvalidGatewayApiKeyToken)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(suffix)
        .map_err(|_| StorageError::InvalidGatewayApiKeyToken)?;
    if decoded.len() != GATEWAY_TOKEN_RANDOM_BYTES {
        return Err(StorageError::InvalidGatewayApiKeyToken);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    use super::display_prefix;

    #[test]
    fn gateway_token_requires_the_versioned_256_bit_format() {
        let valid = format!("a2k_v1_{}", URL_SAFE_NO_PAD.encode([7_u8; 32]));
        assert_eq!(
            display_prefix(&valid.clone().into_bytes().into()).expect("valid token"),
            &valid[..16]
        );
        assert!(display_prefix(&b"a2k_v1_short".to_vec().into()).is_err());
    }
}

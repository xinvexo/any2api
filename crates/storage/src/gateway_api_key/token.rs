use any2api_domain::{GATEWAY_TOKEN_BODY_LEN, GATEWAY_TOKEN_PREFIX, validate_gateway_token};
use secrecy::ExposeSecret;

use crate::{error::StorageError, vault::SecretBytes};

const DISPLAY_PREFIX_BYTES: usize = 16;

pub(crate) fn display_prefix(token: &SecretBytes) -> Result<String, StorageError> {
    let value = std::str::from_utf8(token.expose_secret())
        .map_err(|_| StorageError::InvalidGatewayApiKeyToken)?;
    validate_gateway_token(value.to_owned())
        .map_err(|_| StorageError::InvalidGatewayApiKeyToken)?;
    debug_assert_eq!(
        value.len(),
        GATEWAY_TOKEN_PREFIX.len() + GATEWAY_TOKEN_BODY_LEN
    );
    Ok(value[..DISPLAY_PREFIX_BYTES].to_owned())
}

#[cfg(test)]
mod tests {
    use super::display_prefix;
    use any2api_domain::{GATEWAY_TOKEN_BODY_LEN, GATEWAY_TOKEN_PREFIX};

    #[test]
    fn gateway_token_requires_sk_alphanumeric_format() {
        let valid = format!(
            "{GATEWAY_TOKEN_PREFIX}{}",
            "A7b9".repeat(GATEWAY_TOKEN_BODY_LEN / 4)
        );
        assert_eq!(
            display_prefix(&valid.clone().into_bytes().into()).expect("valid token"),
            &valid[..16]
        );
        assert!(display_prefix(&b"sk-short".to_vec().into()).is_err());
        assert!(display_prefix(&format!("a2k_v1_{}", "a".repeat(43)).into_bytes().into()).is_err());
    }
}

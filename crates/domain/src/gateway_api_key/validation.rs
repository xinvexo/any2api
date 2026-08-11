use thiserror::Error;

pub const GATEWAY_TOKEN_HASH_VERSION: u32 = 2;
pub const GATEWAY_TOKEN_VERSION: u64 = 1;
/// Client-facing gateway tokens always start with this fixed prefix.
pub const GATEWAY_TOKEN_PREFIX: &str = "sk-";
/// URL-safe Base64 without padding for exactly 32 random bytes.
pub const GATEWAY_TOKEN_BODY_LEN: usize = 43;

const MAX_CONFIG_VERSION: u64 = u32::MAX as u64;
const MAX_NAME_CHARS: usize = 100;
const MAX_PREFIX_CHARS: usize = 64;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GatewayApiKeyValidationError {
    #[error("gateway API Key name must not be empty")]
    EmptyName,
    #[error("gateway API Key name must be trimmed")]
    NameNotTrimmed,
    #[error("gateway API Key name is too long")]
    NameTooLong,
    #[error("gateway API Key token prefix is invalid")]
    InvalidTokenPrefix,
    #[error("gateway API Key token is invalid")]
    InvalidToken,
    #[error("gateway API Key version is invalid")]
    InvalidVersion,
    #[error("gateway API Key timestamp is invalid")]
    InvalidTimestamp,
    #[error("gateway API Key id is duplicated")]
    DuplicateId,
    #[error("gateway API Key name is duplicated")]
    DuplicateName,
}

pub(crate) fn validate_name(name: String) -> Result<String, GatewayApiKeyValidationError> {
    if name.trim().is_empty() {
        return Err(GatewayApiKeyValidationError::EmptyName);
    }
    if name.trim() != name {
        return Err(GatewayApiKeyValidationError::NameNotTrimmed);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(GatewayApiKeyValidationError::NameTooLong);
    }
    Ok(name)
}

pub(crate) fn validate_prefix(prefix: String) -> Result<String, GatewayApiKeyValidationError> {
    if prefix.is_empty()
        || prefix.chars().count() > MAX_PREFIX_CHARS
        || !prefix.is_ascii()
        || prefix.chars().any(char::is_control)
    {
        return Err(GatewayApiKeyValidationError::InvalidTokenPrefix);
    }
    Ok(prefix)
}

pub(crate) fn validate_timestamp(value: String) -> Result<String, GatewayApiKeyValidationError> {
    if value.trim().is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(GatewayApiKeyValidationError::InvalidTimestamp);
    }
    Ok(value)
}

pub(crate) const fn valid_version(value: u64) -> bool {
    value > 0 && value <= MAX_CONFIG_VERSION
}

pub(crate) fn next_version(value: u64) -> Result<u64, GatewayApiKeyValidationError> {
    value
        .checked_add(1)
        .filter(|next| *next <= MAX_CONFIG_VERSION)
        .ok_or(GatewayApiKeyValidationError::InvalidVersion)
}

/// Current prefix plus a 32-byte URL-safe Base64 body without padding.
pub fn validate_token(token: String) -> Result<String, GatewayApiKeyValidationError> {
    let Some(body) = token.strip_prefix(GATEWAY_TOKEN_PREFIX) else {
        return Err(GatewayApiKeyValidationError::InvalidToken);
    };
    if body.len() != GATEWAY_TOKEN_BODY_LEN
        || !body.is_ascii()
        || !body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || token.chars().count() > 128
    {
        return Err(GatewayApiKeyValidationError::InvalidToken);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::{GATEWAY_TOKEN_BODY_LEN, GATEWAY_TOKEN_PREFIX, validate_token};

    #[test]
    fn accepts_standard_urlsafe_base64_body() {
        let token = format!(
            "{GATEWAY_TOKEN_PREFIX}{}",
            "A".repeat(GATEWAY_TOKEN_BODY_LEN)
        );
        assert_eq!(
            token.len(),
            GATEWAY_TOKEN_PREFIX.len() + GATEWAY_TOKEN_BODY_LEN
        );
        assert_eq!(validate_token(token.clone()).expect("valid"), token);
    }

    #[test]
    fn rejects_unversioned_and_malformed_tokens() {
        assert!(validate_token(format!("a2k_v1_{}", "a".repeat(43))).is_err());
        assert!(validate_token(format!("{GATEWAY_TOKEN_PREFIX}{}", "a".repeat(10))).is_err());
        assert!(
            validate_token(format!(
                "{GATEWAY_TOKEN_PREFIX}{}",
                "a".repeat(GATEWAY_TOKEN_BODY_LEN - 1) + "+"
            ))
            .is_err()
        );
        assert!(
            validate_token(format!(
                "{GATEWAY_TOKEN_PREFIX}{}",
                "a".repeat(GATEWAY_TOKEN_BODY_LEN - 1) + "="
            ))
            .is_err()
        );
    }
}

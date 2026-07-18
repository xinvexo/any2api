use thiserror::Error;

pub const GATEWAY_TOKEN_HASH_VERSION: u32 = 1;
pub const GATEWAY_TOKEN_VERSION: u64 = 1;
pub const GATEWAY_TOKEN_PREFIX: &str = "a2k_v1_";
pub const GATEWAY_TOKEN_RANDOM_BYTES: usize = 32;

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
    #[error("gateway API Key hash key id is invalid")]
    InvalidHashKeyId,
    #[error("gateway API Key version is invalid")]
    InvalidVersion,
    #[error("gateway API Key timestamp is invalid")]
    InvalidTimestamp,
    #[error("revoked gateway API Key must remain disabled")]
    RevokedEnabled,
    #[error("gateway API Key was revoked")]
    Revoked,
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

pub(crate) fn validate_hash_key_id(value: String) -> Result<String, GatewayApiKeyValidationError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.chars().count() > 128
        || value.chars().any(char::is_control)
    {
        return Err(GatewayApiKeyValidationError::InvalidHashKeyId);
    }
    Ok(value)
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

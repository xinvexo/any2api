use std::fmt;

use super::validation::{
    GATEWAY_TOKEN_HASH_VERSION, GATEWAY_TOKEN_VERSION, GatewayApiKeyValidationError, next_version,
    valid_version, validate_name, validate_prefix, validate_timestamp, validate_token,
};
use crate::{GatewayApiKeyId, RequestsPerMinute};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyDraft {
    name: String,
    requests_per_minute: Option<RequestsPerMinute>,
    enabled: bool,
}

impl GatewayApiKeyDraft {
    pub fn new(
        name: impl Into<String>,
        enabled: bool,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        Ok(Self {
            name: validate_name(name.into())?,
            requests_per_minute: None,
            enabled,
        })
    }

    #[must_use]
    pub const fn with_requests_per_minute(
        mut self,
        requests_per_minute: Option<RequestsPerMinute>,
    ) -> Self {
        self.requests_per_minute = requests_per_minute;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn requests_per_minute(&self) -> Option<RequestsPerMinute> {
        self.requests_per_minute
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct GatewayApiKey {
    id: GatewayApiKeyId,
    name: String,
    token: String,
    token_prefix: String,
    token_hash: [u8; 32],
    hash_version: u32,
    token_version: u64,
    config_version: u64,
    requests_per_minute: Option<RequestsPerMinute>,
    enabled: bool,
    created_at: String,
    last_used_at: Option<String>,
}

impl GatewayApiKey {
    pub fn create(
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
        token: impl Into<String>,
        token_prefix: impl Into<String>,
        token_hash: [u8; 32],
        created_at: impl Into<String>,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        let token = token.into();
        validate_token(&token)?;
        let token_prefix = validate_prefix(token_prefix.into())?;
        let created_at = created_at.into();
        validate_timestamp(&created_at)?;
        Ok(Self {
            id,
            name: draft.name,
            token,
            token_prefix,
            token_hash,
            hash_version: GATEWAY_TOKEN_HASH_VERSION,
            token_version: GATEWAY_TOKEN_VERSION,
            config_version: GATEWAY_TOKEN_VERSION,
            requests_per_minute: draft.requests_per_minute,
            enabled: draft.enabled,
            created_at,
            last_used_at: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
        token: String,
        token_prefix: String,
        token_hash: [u8; 32],
        hash_version: u32,
        token_version: u64,
        config_version: u64,
        created_at: String,
        last_used_at: Option<String>,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        if hash_version != GATEWAY_TOKEN_HASH_VERSION
            || !valid_version(token_version)
            || !valid_version(config_version)
        {
            return Err(GatewayApiKeyValidationError::InvalidVersion);
        }
        validate_token(&token)?;
        let token_prefix = validate_prefix(token_prefix)?;
        if let Some(value) = last_used_at.as_ref() {
            validate_timestamp(value)?;
        }
        validate_timestamp(&created_at)?;
        Ok(Self {
            id,
            name: draft.name,
            token,
            token_prefix,
            token_hash,
            hash_version,
            token_version,
            config_version,
            requests_per_minute: draft.requests_per_minute,
            enabled: draft.enabled,
            created_at,
            last_used_at,
        })
    }

    pub fn updated(&self, draft: GatewayApiKeyDraft) -> Result<Self, GatewayApiKeyValidationError> {
        if self.name == draft.name
            && self.requests_per_minute == draft.requests_per_minute
            && self.enabled == draft.enabled
        {
            return Ok(self.clone());
        }
        Ok(Self {
            name: draft.name,
            requests_per_minute: draft.requests_per_minute,
            enabled: draft.enabled,
            config_version: next_version(self.config_version)?,
            ..self.clone()
        })
    }

    pub fn rotated(
        &self,
        token: impl Into<String>,
        token_prefix: impl Into<String>,
        token_hash: [u8; 32],
    ) -> Result<Self, GatewayApiKeyValidationError> {
        let token = token.into();
        validate_token(&token)?;
        Ok(Self {
            token,
            token_prefix: validate_prefix(token_prefix.into())?,
            token_hash,
            token_version: next_version(self.token_version)?,
            config_version: next_version(self.config_version)?,
            ..self.clone()
        })
    }

    #[must_use]
    pub const fn id(&self) -> GatewayApiKeyId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn name_key(&self) -> String {
        self.name.to_ascii_lowercase()
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    #[must_use]
    pub fn token_prefix(&self) -> &str {
        &self.token_prefix
    }

    #[must_use]
    pub const fn token_hash(&self) -> &[u8; 32] {
        &self.token_hash
    }

    #[must_use]
    pub const fn hash_version(&self) -> u32 {
        self.hash_version
    }

    #[must_use]
    pub const fn token_version(&self) -> u64 {
        self.token_version
    }

    #[must_use]
    pub const fn config_version(&self) -> u64 {
        self.config_version
    }

    #[must_use]
    pub const fn requests_per_minute(&self) -> Option<RequestsPerMinute> {
        self.requests_per_minute
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    #[must_use]
    pub fn last_used_at(&self) -> Option<&str> {
        self.last_used_at.as_deref()
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.enabled
    }
}

impl fmt::Debug for GatewayApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayApiKey")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("token", &"[REDACTED]")
            .field("token_prefix", &self.token_prefix)
            .field("token_hash", &"[REDACTED]")
            .field("hash_version", &self.hash_version)
            .field("token_version", &self.token_version)
            .field("config_version", &self.config_version)
            .field("requests_per_minute", &self.requests_per_minute)
            .field("enabled", &self.enabled)
            .field("created_at", &self.created_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{GatewayApiKey, GatewayApiKeyDraft};
    use crate::{GatewayApiKeyId, RequestsPerMinute};

    fn sample_token(seed: char) -> String {
        format!(
            "{}{}",
            crate::GATEWAY_TOKEN_PREFIX,
            seed.to_string().repeat(crate::GATEWAY_TOKEN_BODY_LEN)
        )
    }

    fn key() -> GatewayApiKey {
        let token = sample_token('a');
        GatewayApiKey::create(
            GatewayApiKeyId::new(),
            GatewayApiKeyDraft::new("Desktop", true).expect("draft"),
            token.clone(),
            &token[..16],
            [7; 32],
            "2026-07-19 00:00:00",
        )
        .expect("key")
    }

    #[test]
    fn create_stores_plaintext_and_redacts_debug() {
        let key = key();
        assert!(key.token().starts_with(crate::GATEWAY_TOKEN_PREFIX));
        assert!(format!("{key:?}").contains("[REDACTED]"));
        assert!(!format!("{key:?}").contains(key.token()));
    }

    #[test]
    fn rpm_updates_config_version_and_rotation_preserves_the_limit() {
        let rpm = RequestsPerMinute::new(120).expect("RPM");
        let limited = key()
            .updated(
                GatewayApiKeyDraft::new("Desktop", true)
                    .expect("draft")
                    .with_requests_per_minute(Some(rpm)),
            )
            .expect("limited key");
        assert_eq!(limited.config_version(), 2);
        assert_eq!(limited.requests_per_minute(), Some(rpm));

        let token = sample_token('b');
        let rotated = limited
            .rotated(token.clone(), &token[..16], [9; 32])
            .expect("rotated");
        assert_eq!(rotated.token_version(), 2);
        assert_eq!(rotated.config_version(), 3);
        assert_eq!(rotated.token(), token);
        assert_eq!(rotated.requests_per_minute(), Some(rpm));
        assert!(rotated.is_active());
    }
}

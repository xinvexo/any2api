use std::fmt;

use crate::{
    GatewayApiKeyId,
    gateway_api_key_validation::{
        GATEWAY_TOKEN_HASH_VERSION, GATEWAY_TOKEN_VERSION, GatewayApiKeyValidationError,
        next_version, valid_version, validate_hash_key_id, validate_name, validate_prefix,
        validate_timestamp, validate_token,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyDraft {
    name: String,
    enabled: bool,
}

impl GatewayApiKeyDraft {
    pub fn new(
        name: impl Into<String>,
        enabled: bool,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        Ok(Self {
            name: validate_name(name.into())?,
            enabled,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
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
    hash_key_id: String,
    token_version: u64,
    config_version: u64,
    enabled: bool,
    revoked_at: Option<String>,
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
        hash_key_id: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        let token = validate_token(token.into())?;
        let token_prefix = validate_prefix(token_prefix.into())?;
        let hash_key_id = validate_hash_key_id(hash_key_id.into())?;
        let created_at = validate_timestamp(created_at.into())?;
        Ok(Self {
            id,
            name: draft.name,
            token,
            token_prefix,
            token_hash,
            hash_version: GATEWAY_TOKEN_HASH_VERSION,
            hash_key_id,
            token_version: GATEWAY_TOKEN_VERSION,
            config_version: GATEWAY_TOKEN_VERSION,
            enabled: draft.enabled,
            revoked_at: None,
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
        hash_key_id: String,
        token_version: u64,
        config_version: u64,
        revoked_at: Option<String>,
        created_at: String,
        last_used_at: Option<String>,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        if hash_version != GATEWAY_TOKEN_HASH_VERSION
            || !valid_version(token_version)
            || !valid_version(config_version)
        {
            return Err(GatewayApiKeyValidationError::InvalidVersion);
        }
        let token = validate_token(token)?;
        let token_prefix = validate_prefix(token_prefix)?;
        let hash_key_id = validate_hash_key_id(hash_key_id)?;
        if created_at.trim().is_empty() {
            return Err(GatewayApiKeyValidationError::InvalidTimestamp);
        }
        if revoked_at.is_some() && draft.enabled {
            return Err(GatewayApiKeyValidationError::RevokedEnabled);
        }
        if let Some(value) = revoked_at.as_ref() {
            validate_timestamp(value.clone())?;
        }
        if let Some(value) = last_used_at.as_ref() {
            validate_timestamp(value.clone())?;
        }
        let created_at = validate_timestamp(created_at)?;
        Ok(Self {
            id,
            name: draft.name,
            token,
            token_prefix,
            token_hash,
            hash_version,
            hash_key_id,
            token_version,
            config_version,
            enabled: draft.enabled,
            revoked_at,
            created_at,
            last_used_at,
        })
    }

    pub fn updated(&self, draft: GatewayApiKeyDraft) -> Result<Self, GatewayApiKeyValidationError> {
        if self.name == draft.name && self.enabled == draft.enabled {
            return Ok(self.clone());
        }
        if self.is_revoked() && draft.enabled {
            return Err(GatewayApiKeyValidationError::Revoked);
        }
        Ok(Self {
            name: draft.name,
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
        hash_key_id: impl Into<String>,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        if self.is_revoked() {
            return Err(GatewayApiKeyValidationError::Revoked);
        }
        Ok(Self {
            token: validate_token(token.into())?,
            token_prefix: validate_prefix(token_prefix.into())?,
            token_hash,
            hash_key_id: validate_hash_key_id(hash_key_id.into())?,
            token_version: next_version(self.token_version)?,
            config_version: next_version(self.config_version)?,
            ..self.clone()
        })
    }

    pub fn revoked(
        &self,
        revoked_at: impl Into<String>,
    ) -> Result<Self, GatewayApiKeyValidationError> {
        if self.is_revoked() {
            return Ok(self.clone());
        }
        let revoked_at = validate_timestamp(revoked_at.into())?;
        Ok(Self {
            enabled: false,
            revoked_at: Some(revoked_at),
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
    pub fn hash_key_id(&self) -> &str {
        &self.hash_key_id
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
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn revoked_at(&self) -> Option<&str> {
        self.revoked_at.as_deref()
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
    pub const fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.enabled && self.revoked_at.is_none()
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
            .field("hash_key_id", &self.hash_key_id)
            .field("token_version", &self.token_version)
            .field("config_version", &self.config_version)
            .field("enabled", &self.enabled)
            .field("revoked_at", &self.revoked_at)
            .field("created_at", &self.created_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{GatewayApiKey, GatewayApiKeyDraft};
    use crate::GatewayApiKeyId;

    // 43-char base64url body matches the production a2k_v1_ format length.
    fn sample_token(seed: char) -> String {
        format!("a2k_v1_{}", seed.to_string().repeat(43))
    }

    fn key() -> GatewayApiKey {
        let token = sample_token('a');
        GatewayApiKey::create(
            GatewayApiKeyId::new(),
            GatewayApiKeyDraft::new("Desktop", true).expect("draft"),
            token.clone(),
            &token[..16],
            [7; 32],
            "gk1_test",
            "2026-07-19 00:00:00",
        )
        .expect("key")
    }

    #[test]
    fn create_stores_plaintext_and_redacts_debug() {
        let key = key();
        assert!(key.token().starts_with("a2k_v1_"));
        assert!(format!("{key:?}").contains("[REDACTED]"));
        assert!(!format!("{key:?}").contains(key.token()));
    }

    #[test]
    fn rotate_and_soft_revoke_still_work_in_domain() {
        let token = sample_token('b');
        let rotated = key()
            .rotated(token.clone(), &token[..16], [9; 32], "gk1_test")
            .expect("rotated");
        assert_eq!(rotated.token_version(), 2);
        assert_eq!(rotated.token(), token);
        let revoked = rotated.revoked("2026-07-19 00:00:00").expect("revoked");
        assert!(revoked.is_revoked());
        assert!(!revoked.is_active());
        assert!(
            revoked
                .updated(GatewayApiKeyDraft::new("Desktop", true).expect("draft"))
                .is_err()
        );
    }
}

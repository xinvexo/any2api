use crate::{GatewayApiKey, GatewayApiKeyId, GatewayApiKeyValidationError};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GatewayApiKeyConfiguration {
    keys: Vec<GatewayApiKey>,
}

impl GatewayApiKeyConfiguration {
    pub fn new(mut keys: Vec<GatewayApiKey>) -> Result<Self, GatewayApiKeyValidationError> {
        for (index, key) in keys.iter().enumerate() {
            if keys[..index].iter().any(|other| other.id() == key.id()) {
                return Err(GatewayApiKeyValidationError::DuplicateId);
            }
            if keys[..index]
                .iter()
                .any(|other| other.name_key() == key.name_key())
            {
                return Err(GatewayApiKeyValidationError::DuplicateName);
            }
        }
        keys.sort_by(|left, right| left.name().cmp(right.name()));
        Ok(Self { keys })
    }

    #[must_use]
    pub const fn initial() -> Self {
        Self { keys: Vec::new() }
    }

    #[must_use]
    pub fn keys(&self) -> &[GatewayApiKey] {
        &self.keys
    }

    #[must_use]
    pub fn get(&self, id: GatewayApiKeyId) -> Option<&GatewayApiKey> {
        self.keys.iter().find(|key| key.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::GatewayApiKeyConfiguration;
    use crate::{GatewayApiKey, GatewayApiKeyDraft, GatewayApiKeyId};

    fn key(name: &str) -> GatewayApiKey {
        let token = format!("a2k_v1_{}", "a".repeat(43));
        GatewayApiKey::create(
            GatewayApiKeyId::new(),
            GatewayApiKeyDraft::new(name, true).expect("draft"),
            token.clone(),
            &token[..16],
            [7; 32],
            "gk1_test",
            "2026-07-19 00:00:00",
        )
        .expect("key")
    }

    #[test]
    fn configuration_rejects_duplicate_names() {
        let first = key("Same");
        let second = key("same");
        assert!(GatewayApiKeyConfiguration::new(vec![first, second]).is_err());
    }
}

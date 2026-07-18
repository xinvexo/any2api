use any2api_domain::{
    GatewayApiKey, GatewayApiKeyConfiguration, GatewayApiKeyDraft, GatewayApiKeyId,
    GatewayApiKeyValidationError,
};
use secrecy::ExposeSecret;

use crate::{
    error::StorageError, gateway_api_key_token::display_prefix,
    gateway_api_key_verifier::GatewayApiKeyVerifier, vault::SecretBytes,
};

pub(crate) enum GatewayApiKeyMutation {
    Create {
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
        token: SecretBytes,
        created_at: String,
    },
    Update {
        id: GatewayApiKeyId,
        expected_config_version: u64,
        draft: GatewayApiKeyDraft,
    },
    Rotate {
        id: GatewayApiKeyId,
        expected_config_version: u64,
        expected_token_version: u64,
        token: SecretBytes,
    },
    Revoke {
        id: GatewayApiKeyId,
        expected_config_version: u64,
        revoked_at: String,
    },
}

pub(crate) enum GatewayApiKeyDatabaseChange {
    Create(GatewayApiKey),
    Update(GatewayApiKey),
    Rotate(GatewayApiKey),
    Revoke(GatewayApiKey),
}

pub(crate) struct PreparedGatewayApiKeyMutation {
    configuration: GatewayApiKeyConfiguration,
    change: GatewayApiKeyDatabaseChange,
}

impl PreparedGatewayApiKeyMutation {
    pub(crate) const fn new(
        configuration: GatewayApiKeyConfiguration,
        change: GatewayApiKeyDatabaseChange,
    ) -> Self {
        Self {
            configuration,
            change,
        }
    }

    pub(crate) const fn change(&self) -> &GatewayApiKeyDatabaseChange {
        &self.change
    }

    pub(crate) fn into_configuration(self) -> GatewayApiKeyConfiguration {
        self.configuration
    }
}

pub(crate) fn prepare(
    current: &GatewayApiKeyConfiguration,
    verifier: &GatewayApiKeyVerifier,
    mutation: GatewayApiKeyMutation,
) -> Result<Option<PreparedGatewayApiKeyMutation>, StorageError> {
    match mutation {
        GatewayApiKeyMutation::Create {
            id,
            draft,
            token,
            created_at,
        } => create(current, verifier, id, draft, token, created_at).map(Some),
        GatewayApiKeyMutation::Update {
            id,
            expected_config_version,
            draft,
        } => update(current, id, expected_config_version, draft),
        GatewayApiKeyMutation::Rotate {
            id,
            expected_config_version,
            expected_token_version,
            token,
        } => rotate(
            current,
            verifier,
            id,
            expected_config_version,
            expected_token_version,
            token,
        )
        .map(Some),
        GatewayApiKeyMutation::Revoke {
            id,
            expected_config_version,
            revoked_at,
        } => revoke(current, id, expected_config_version, revoked_at),
    }
}

fn create(
    current: &GatewayApiKeyConfiguration,
    verifier: &GatewayApiKeyVerifier,
    id: GatewayApiKeyId,
    draft: GatewayApiKeyDraft,
    token: SecretBytes,
    created_at: String,
) -> Result<PreparedGatewayApiKeyMutation, StorageError> {
    let key = GatewayApiKey::create(
        id,
        draft,
        display_prefix(&token)?,
        verifier.hash(token.expose_secret()),
        verifier.key_id(),
        created_at,
    )?;
    let configuration = append(current, key.clone())?;
    Ok(PreparedGatewayApiKeyMutation::new(
        configuration,
        GatewayApiKeyDatabaseChange::Create(key),
    ))
}

fn update(
    current: &GatewayApiKeyConfiguration,
    id: GatewayApiKeyId,
    expected_config_version: u64,
    draft: GatewayApiKeyDraft,
) -> Result<Option<PreparedGatewayApiKeyMutation>, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::GatewayApiKeyNotFound(id))?;
    require_config_version(existing.config_version(), expected_config_version)?;
    let updated = existing.updated(draft)?;
    if &updated == existing {
        return Ok(None);
    }
    let configuration = replace(current, updated.clone())?;
    Ok(Some(PreparedGatewayApiKeyMutation::new(
        configuration,
        GatewayApiKeyDatabaseChange::Update(updated),
    )))
}

fn rotate(
    current: &GatewayApiKeyConfiguration,
    verifier: &GatewayApiKeyVerifier,
    id: GatewayApiKeyId,
    expected_config_version: u64,
    expected_token_version: u64,
    token: SecretBytes,
) -> Result<PreparedGatewayApiKeyMutation, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::GatewayApiKeyNotFound(id))?;
    require_config_version(existing.config_version(), expected_config_version)?;
    if existing.token_version() != expected_token_version {
        return Err(StorageError::GatewayApiKeyTokenVersionConflict {
            expected: expected_token_version,
            actual: existing.token_version(),
        });
    }
    let rotated = existing.rotated(
        display_prefix(&token)?,
        verifier.hash(token.expose_secret()),
        verifier.key_id(),
    )?;
    let configuration = replace(current, rotated.clone())?;
    Ok(PreparedGatewayApiKeyMutation::new(
        configuration,
        GatewayApiKeyDatabaseChange::Rotate(rotated),
    ))
}

fn revoke(
    current: &GatewayApiKeyConfiguration,
    id: GatewayApiKeyId,
    expected_config_version: u64,
    revoked_at: String,
) -> Result<Option<PreparedGatewayApiKeyMutation>, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::GatewayApiKeyNotFound(id))?;
    require_config_version(existing.config_version(), expected_config_version)?;
    let revoked = existing.revoked(revoked_at)?;
    if &revoked == existing {
        return Ok(None);
    }
    let configuration = replace(current, revoked.clone())?;
    Ok(Some(PreparedGatewayApiKeyMutation::new(
        configuration,
        GatewayApiKeyDatabaseChange::Revoke(revoked),
    )))
}

fn append(
    current: &GatewayApiKeyConfiguration,
    key: GatewayApiKey,
) -> Result<GatewayApiKeyConfiguration, StorageError> {
    let mut keys = current.keys().to_vec();
    keys.push(key);
    GatewayApiKeyConfiguration::new(keys).map_err(map_validation)
}

fn replace(
    current: &GatewayApiKeyConfiguration,
    updated: GatewayApiKey,
) -> Result<GatewayApiKeyConfiguration, StorageError> {
    let keys = current
        .keys()
        .iter()
        .map(|key| {
            if key.id() == updated.id() {
                updated.clone()
            } else {
                key.clone()
            }
        })
        .collect();
    GatewayApiKeyConfiguration::new(keys).map_err(map_validation)
}

fn require_config_version(current: u64, expected: u64) -> Result<(), StorageError> {
    if current == expected {
        Ok(())
    } else {
        Err(StorageError::GatewayApiKeyVersionConflict {
            expected,
            actual: current,
        })
    }
}

fn map_validation(error: GatewayApiKeyValidationError) -> StorageError {
    match error {
        GatewayApiKeyValidationError::DuplicateName => StorageError::GatewayApiKeyNameConflict,
        GatewayApiKeyValidationError::Revoked => StorageError::GatewayApiKeyRevoked,
        GatewayApiKeyValidationError::DuplicateId => StorageError::CorruptConfiguration,
        other => StorageError::GatewayApiKeyValidation(other),
    }
}

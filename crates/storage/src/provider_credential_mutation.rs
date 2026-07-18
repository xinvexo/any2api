use any2api_domain::{
    CredentialId, ProviderCredential, ProviderCredentialConfiguration, ProviderCredentialDraft,
    ProviderCredentialValidationError, ProviderEndpointConfiguration, ProviderEndpointId,
    ProxyConfiguration,
};

use crate::{
    error::StorageError,
    provider_credential_secret_mutation::{CredentialSecretMutationContext, create, rotate_secret},
    vault::{SecretBytes, SecretEnvelope, SecretVault},
};

pub(crate) enum ProviderCredentialMutation {
    Create {
        id: CredentialId,
        endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        api_key: SecretBytes,
    },
    Update {
        id: CredentialId,
        expected_config_version: u64,
        draft: ProviderCredentialDraft,
    },
    RotateSecret {
        id: CredentialId,
        expected_config_version: u64,
        expected_secret_version: u64,
        api_key: SecretBytes,
    },
    Delete {
        id: CredentialId,
        expected_config_version: u64,
    },
}

pub(crate) enum ProviderCredentialDatabaseChange {
    Create {
        credential: ProviderCredential,
        envelope: SecretEnvelope,
    },
    Update(ProviderCredential),
    RotateSecret {
        credential: ProviderCredential,
        envelope: SecretEnvelope,
    },
    Delete(CredentialId),
}

pub(crate) struct PreparedProviderCredentialMutation {
    configuration: ProviderCredentialConfiguration,
    change: ProviderCredentialDatabaseChange,
}

impl PreparedProviderCredentialMutation {
    pub(crate) const fn new(
        configuration: ProviderCredentialConfiguration,
        change: ProviderCredentialDatabaseChange,
    ) -> Self {
        Self {
            configuration,
            change,
        }
    }

    pub(crate) const fn change(&self) -> &ProviderCredentialDatabaseChange {
        &self.change
    }

    pub(crate) fn into_configuration(self) -> ProviderCredentialConfiguration {
        self.configuration
    }
}

pub(crate) fn prepare_provider_credential_mutation(
    current: &ProviderCredentialConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
    vault: &SecretVault,
    mutation: ProviderCredentialMutation,
) -> Result<Option<PreparedProviderCredentialMutation>, StorageError> {
    let secret_context = CredentialSecretMutationContext::new(current, endpoints, proxies, vault);
    match mutation {
        ProviderCredentialMutation::Create {
            id,
            endpoint_id,
            draft,
            api_key,
        } => create(&secret_context, id, endpoint_id, draft, api_key).map(Some),
        ProviderCredentialMutation::Update {
            id,
            expected_config_version,
            draft,
        } => update(
            current,
            endpoints,
            proxies,
            id,
            expected_config_version,
            draft,
        ),
        ProviderCredentialMutation::RotateSecret {
            id,
            expected_config_version,
            expected_secret_version,
            api_key,
        } => rotate_secret(
            &secret_context,
            id,
            expected_config_version,
            expected_secret_version,
            api_key,
        )
        .map(Some),
        ProviderCredentialMutation::Delete {
            id,
            expected_config_version,
        } => delete(current, endpoints, proxies, id, expected_config_version).map(Some),
    }
}

fn update(
    current: &ProviderCredentialConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
    id: CredentialId,
    expected_config_version: u64,
    draft: ProviderCredentialDraft,
) -> Result<Option<PreparedProviderCredentialMutation>, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::ProviderCredentialNotFound(id))?;
    require_config_version(existing, expected_config_version)?;
    let updated = existing.updated(draft)?;
    if &updated == existing {
        return Ok(None);
    }
    let configuration = replace(current, endpoints, proxies, updated.clone())?;
    Ok(Some(PreparedProviderCredentialMutation {
        configuration,
        change: ProviderCredentialDatabaseChange::Update(updated),
    }))
}

fn delete(
    current: &ProviderCredentialConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
    id: CredentialId,
    expected_config_version: u64,
) -> Result<PreparedProviderCredentialMutation, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::ProviderCredentialNotFound(id))?;
    require_config_version(existing, expected_config_version)?;
    let credentials = current
        .credentials()
        .iter()
        .filter(|credential| credential.id() != id)
        .cloned()
        .collect();
    let configuration = ProviderCredentialConfiguration::new(credentials, endpoints, proxies)
        .map_err(map_validation)?;
    Ok(PreparedProviderCredentialMutation {
        configuration,
        change: ProviderCredentialDatabaseChange::Delete(id),
    })
}

pub(crate) fn replace(
    current: &ProviderCredentialConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
    updated: ProviderCredential,
) -> Result<ProviderCredentialConfiguration, StorageError> {
    let credentials = current
        .credentials()
        .iter()
        .map(|credential| {
            if credential.id() == updated.id() {
                updated.clone()
            } else {
                credential.clone()
            }
        })
        .collect();
    ProviderCredentialConfiguration::new(credentials, endpoints, proxies).map_err(map_validation)
}

pub(crate) fn require_config_version(
    credential: &ProviderCredential,
    expected: u64,
) -> Result<(), StorageError> {
    if credential.config_version() == expected {
        Ok(())
    } else {
        Err(StorageError::ProviderCredentialVersionConflict {
            expected,
            actual: credential.config_version(),
        })
    }
}

pub(crate) fn map_validation(error: ProviderCredentialValidationError) -> StorageError {
    match error {
        ProviderCredentialValidationError::DuplicateLabel => {
            StorageError::ProviderCredentialLabelConflict
        }
        ProviderCredentialValidationError::MissingProviderEndpoint
        | ProviderCredentialValidationError::MissingProxyProfile => {
            StorageError::CorruptConfiguration
        }
        other => StorageError::ProviderCredentialValidation(other),
    }
}

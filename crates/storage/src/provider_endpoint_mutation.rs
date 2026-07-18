use any2api_domain::{
    ProviderCredentialConfiguration, ProviderEndpoint, ProviderEndpointConfiguration,
    ProviderEndpointDraft, ProviderEndpointId, ProviderEndpointValidationError, ProxyConfiguration,
};

use crate::error::StorageError;

pub(crate) enum ProviderEndpointMutation {
    Create {
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
    },
    Update {
        id: ProviderEndpointId,
        expected_config_version: u64,
        draft: ProviderEndpointDraft,
    },
    Delete {
        id: ProviderEndpointId,
    },
}

pub(crate) enum ProviderEndpointDatabaseChange {
    Create(ProviderEndpoint),
    Update(ProviderEndpoint),
    Delete(ProviderEndpointId),
}

pub(crate) struct PreparedProviderEndpointMutation {
    provider_endpoints: ProviderEndpointConfiguration,
    provider_credentials: ProviderCredentialConfiguration,
    change: ProviderEndpointDatabaseChange,
    bump_credential_generations: bool,
}

impl PreparedProviderEndpointMutation {
    pub(crate) const fn change(&self) -> &ProviderEndpointDatabaseChange {
        &self.change
    }

    pub(crate) const fn endpoint_id(&self) -> ProviderEndpointId {
        match &self.change {
            ProviderEndpointDatabaseChange::Create(endpoint)
            | ProviderEndpointDatabaseChange::Update(endpoint) => endpoint.id(),
            ProviderEndpointDatabaseChange::Delete(id) => *id,
        }
    }

    pub(crate) const fn bump_credential_generations(&self) -> bool {
        self.bump_credential_generations
    }

    pub(crate) fn into_configurations(
        self,
    ) -> (
        ProviderEndpointConfiguration,
        ProviderCredentialConfiguration,
    ) {
        (self.provider_endpoints, self.provider_credentials)
    }
}

pub(crate) fn prepare_provider_endpoint_mutation(
    current: &ProviderEndpointConfiguration,
    credentials: &ProviderCredentialConfiguration,
    proxies: &ProxyConfiguration,
    mutation: ProviderEndpointMutation,
) -> Result<Option<PreparedProviderEndpointMutation>, StorageError> {
    match mutation {
        ProviderEndpointMutation::Create { id, draft } => {
            create(current, credentials, id, draft).map(Some)
        }
        ProviderEndpointMutation::Update {
            id,
            expected_config_version,
            draft,
        } => update(
            current,
            credentials,
            proxies,
            id,
            expected_config_version,
            draft,
        ),
        ProviderEndpointMutation::Delete { id } => delete(current, credentials, id).map(Some),
    }
}

fn create(
    current: &ProviderEndpointConfiguration,
    credentials: &ProviderCredentialConfiguration,
    id: ProviderEndpointId,
    draft: ProviderEndpointDraft,
) -> Result<PreparedProviderEndpointMutation, StorageError> {
    let endpoint = ProviderEndpoint::create(id, draft)?;
    let mut endpoints = current.endpoints().to_vec();
    endpoints.push(endpoint.clone());
    let configuration = ProviderEndpointConfiguration::new(endpoints).map_err(map_validation)?;
    Ok(PreparedProviderEndpointMutation {
        provider_endpoints: configuration,
        provider_credentials: credentials.clone(),
        change: ProviderEndpointDatabaseChange::Create(endpoint),
        bump_credential_generations: false,
    })
}

fn update(
    current: &ProviderEndpointConfiguration,
    credentials: &ProviderCredentialConfiguration,
    proxies: &ProxyConfiguration,
    id: ProviderEndpointId,
    expected_config_version: u64,
    draft: ProviderEndpointDraft,
) -> Result<Option<PreparedProviderEndpointMutation>, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::ProviderEndpointNotFound(id))?;
    if existing.config_version() != expected_config_version {
        return Err(StorageError::ProviderEndpointVersionConflict {
            expected: expected_config_version,
            actual: existing.config_version(),
        });
    }
    let updated = existing.updated(draft)?;
    if &updated == existing {
        return Ok(None);
    }
    let has_credentials = credentials.references_endpoint(id);
    if has_credentials
        && (existing.provider_kind() != updated.provider_kind()
            || existing.protocol_dialect() != updated.protocol_dialect())
    {
        return Err(StorageError::ProviderEndpointIdentityInUse);
    }
    let base_url_changed = existing.base_url() != updated.base_url();
    let endpoints = current
        .endpoints()
        .iter()
        .map(|endpoint| {
            if endpoint.id() == id {
                updated.clone()
            } else {
                endpoint.clone()
            }
        })
        .collect();
    let configuration = ProviderEndpointConfiguration::new(endpoints).map_err(map_validation)?;
    let provider_credentials = if has_credentials && base_url_changed {
        credentials
            .with_endpoint_generation_incremented(id, &configuration, proxies)
            .map_err(|_| StorageError::CorruptConfiguration)?
    } else {
        credentials.clone()
    };
    Ok(Some(PreparedProviderEndpointMutation {
        provider_endpoints: configuration,
        provider_credentials,
        change: ProviderEndpointDatabaseChange::Update(updated),
        bump_credential_generations: has_credentials && base_url_changed,
    }))
}

fn delete(
    current: &ProviderEndpointConfiguration,
    credentials: &ProviderCredentialConfiguration,
    id: ProviderEndpointId,
) -> Result<PreparedProviderEndpointMutation, StorageError> {
    if current.get(id).is_none() {
        return Err(StorageError::ProviderEndpointNotFound(id));
    }
    if credentials.references_endpoint(id) {
        return Err(StorageError::ProviderEndpointInUse);
    }
    let endpoints = current
        .endpoints()
        .iter()
        .filter(|endpoint| endpoint.id() != id)
        .cloned()
        .collect();
    let configuration = ProviderEndpointConfiguration::new(endpoints).map_err(map_validation)?;
    Ok(PreparedProviderEndpointMutation {
        provider_endpoints: configuration,
        provider_credentials: credentials.clone(),
        change: ProviderEndpointDatabaseChange::Delete(id),
        bump_credential_generations: false,
    })
}

fn map_validation(error: ProviderEndpointValidationError) -> StorageError {
    match error {
        ProviderEndpointValidationError::DuplicateName => {
            StorageError::ProviderEndpointNameConflict
        }
        other => StorageError::ProviderEndpointValidation(other),
    }
}

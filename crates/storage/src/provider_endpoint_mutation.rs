use any2api_domain::{
    ProviderEndpoint, ProviderEndpointConfiguration, ProviderEndpointDraft, ProviderEndpointId,
    ProviderEndpointValidationError,
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
    configuration: ProviderEndpointConfiguration,
    change: ProviderEndpointDatabaseChange,
}

impl PreparedProviderEndpointMutation {
    pub(crate) const fn change(&self) -> &ProviderEndpointDatabaseChange {
        &self.change
    }

    pub(crate) fn into_configuration(self) -> ProviderEndpointConfiguration {
        self.configuration
    }
}

pub(crate) fn prepare_provider_endpoint_mutation(
    current: &ProviderEndpointConfiguration,
    mutation: ProviderEndpointMutation,
) -> Result<Option<PreparedProviderEndpointMutation>, StorageError> {
    match mutation {
        ProviderEndpointMutation::Create { id, draft } => create(current, id, draft).map(Some),
        ProviderEndpointMutation::Update {
            id,
            expected_config_version,
            draft,
        } => update(current, id, expected_config_version, draft),
        ProviderEndpointMutation::Delete { id } => delete(current, id).map(Some),
    }
}

fn create(
    current: &ProviderEndpointConfiguration,
    id: ProviderEndpointId,
    draft: ProviderEndpointDraft,
) -> Result<PreparedProviderEndpointMutation, StorageError> {
    let endpoint = ProviderEndpoint::create(id, draft)?;
    let mut endpoints = current.endpoints().to_vec();
    endpoints.push(endpoint.clone());
    let configuration = ProviderEndpointConfiguration::new(endpoints).map_err(map_validation)?;
    Ok(PreparedProviderEndpointMutation {
        configuration,
        change: ProviderEndpointDatabaseChange::Create(endpoint),
    })
}

fn update(
    current: &ProviderEndpointConfiguration,
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
    Ok(Some(PreparedProviderEndpointMutation {
        configuration,
        change: ProviderEndpointDatabaseChange::Update(updated),
    }))
}

fn delete(
    current: &ProviderEndpointConfiguration,
    id: ProviderEndpointId,
) -> Result<PreparedProviderEndpointMutation, StorageError> {
    if current.get(id).is_none() {
        return Err(StorageError::ProviderEndpointNotFound(id));
    }
    let endpoints = current
        .endpoints()
        .iter()
        .filter(|endpoint| endpoint.id() != id)
        .cloned()
        .collect();
    let configuration = ProviderEndpointConfiguration::new(endpoints).map_err(map_validation)?;
    Ok(PreparedProviderEndpointMutation {
        configuration,
        change: ProviderEndpointDatabaseChange::Delete(id),
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

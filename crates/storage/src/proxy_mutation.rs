use any2api_domain::{
    ProviderCredentialConfiguration, ProxyConfiguration, ProxyDraft, ProxyProfile, ProxyProfileId,
    ProxyValidationError,
};

use crate::error::StorageError;

pub(crate) enum ProxyMutation {
    Create {
        id: ProxyProfileId,
        draft: ProxyDraft,
    },
    Update {
        id: ProxyProfileId,
        draft: ProxyDraft,
    },
    Delete {
        id: ProxyProfileId,
    },
    SetGlobal {
        id: ProxyProfileId,
    },
}

pub(crate) enum DatabaseChange {
    Create(ProxyProfile),
    Update(ProxyProfile),
    Delete(ProxyProfileId),
    SetGlobal(ProxyProfileId),
}

pub(crate) struct PreparedMutation {
    configuration: ProxyConfiguration,
    change: DatabaseChange,
}

impl PreparedMutation {
    pub(crate) const fn change(&self) -> &DatabaseChange {
        &self.change
    }

    pub(crate) fn into_configuration(self) -> ProxyConfiguration {
        self.configuration
    }
}

pub(crate) fn prepare_mutation(
    current: &ProxyConfiguration,
    credentials: &ProviderCredentialConfiguration,
    mutation: ProxyMutation,
) -> Result<Option<PreparedMutation>, StorageError> {
    match mutation {
        ProxyMutation::Create { id, draft } => create(current, id, draft).map(Some),
        ProxyMutation::Update { id, draft } => update(current, id, draft),
        ProxyMutation::Delete { id } => delete(current, credentials, id).map(Some),
        ProxyMutation::SetGlobal { id } => set_global(current, id),
    }
}

fn create(
    current: &ProxyConfiguration,
    id: ProxyProfileId,
    draft: ProxyDraft,
) -> Result<PreparedMutation, StorageError> {
    let profile = ProxyProfile::create(id, draft)?;
    let mut profiles = current.profiles().to_vec();
    profiles.push(profile.clone());
    let configuration = ProxyConfiguration::new(profiles, current.global_proxy_id())
        .map_err(map_configuration_error)?;

    Ok(PreparedMutation {
        configuration,
        change: DatabaseChange::Create(profile),
    })
}

fn update(
    current: &ProxyConfiguration,
    id: ProxyProfileId,
    draft: ProxyDraft,
) -> Result<Option<PreparedMutation>, StorageError> {
    let existing = current.get(id).ok_or(StorageError::ProxyNotFound(id))?;
    if existing.is_built_in() {
        return Err(StorageError::ProxyProtected);
    }
    let updated = existing.updated(draft)?;
    if &updated == existing {
        return Ok(None);
    }
    if current.global_proxy_id() == id && !updated.enabled() {
        return Err(StorageError::ProxyInUse);
    }
    let profiles = current
        .profiles()
        .iter()
        .map(|profile| {
            if profile.id() == id {
                updated.clone()
            } else {
                profile.clone()
            }
        })
        .collect();
    let configuration = ProxyConfiguration::new(profiles, current.global_proxy_id())
        .map_err(map_configuration_error)?;

    Ok(Some(PreparedMutation {
        configuration,
        change: DatabaseChange::Update(updated),
    }))
}

fn delete(
    current: &ProxyConfiguration,
    credentials: &ProviderCredentialConfiguration,
    id: ProxyProfileId,
) -> Result<PreparedMutation, StorageError> {
    let existing = current.get(id).ok_or(StorageError::ProxyNotFound(id))?;
    if existing.is_built_in() {
        return Err(StorageError::ProxyProtected);
    }
    if current.global_proxy_id() == id {
        return Err(StorageError::ProxyInUse);
    }
    if credentials.references_proxy(id) {
        return Err(StorageError::ProxyReferenced);
    }
    let profiles = current
        .profiles()
        .iter()
        .filter(|profile| profile.id() != id)
        .cloned()
        .collect();
    let configuration = ProxyConfiguration::new(profiles, current.global_proxy_id())
        .map_err(map_configuration_error)?;

    Ok(PreparedMutation {
        configuration,
        change: DatabaseChange::Delete(id),
    })
}

fn set_global(
    current: &ProxyConfiguration,
    id: ProxyProfileId,
) -> Result<Option<PreparedMutation>, StorageError> {
    if current.global_proxy_id() == id {
        return Ok(None);
    }
    let profile = current.get(id).ok_or(StorageError::ProxyNotFound(id))?;
    if !profile.enabled() {
        return Err(StorageError::ProxyDisabled);
    }
    let configuration = ProxyConfiguration::new(current.profiles().to_vec(), id)
        .map_err(map_configuration_error)?;

    Ok(Some(PreparedMutation {
        configuration,
        change: DatabaseChange::SetGlobal(id),
    }))
}

fn map_configuration_error(error: ProxyValidationError) -> StorageError {
    match error {
        ProxyValidationError::DuplicateName => StorageError::ProxyNameConflict,
        ProxyValidationError::GlobalProxyDisabled => StorageError::ProxyInUse,
        other => StorageError::ProxyValidation(other),
    }
}

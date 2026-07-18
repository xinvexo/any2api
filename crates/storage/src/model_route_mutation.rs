use any2api_domain::{
    ModelRoute, ModelRouteConfiguration, ModelRouteDraft, ModelRouteId, ModelRouteValidationError,
    ProviderEndpointConfiguration,
};

use crate::error::StorageError;

pub(crate) enum ModelRouteMutation {
    Create {
        id: ModelRouteId,
        draft: ModelRouteDraft,
    },
    Update {
        id: ModelRouteId,
        expected_config_version: u64,
        draft: ModelRouteDraft,
    },
    Delete {
        id: ModelRouteId,
        expected_config_version: u64,
    },
}

pub(crate) enum ModelRouteDatabaseChange {
    Create(ModelRoute),
    Update(ModelRoute),
    Delete(ModelRouteId),
}

pub(crate) struct PreparedModelRouteMutation {
    model_routes: ModelRouteConfiguration,
    change: ModelRouteDatabaseChange,
}

impl PreparedModelRouteMutation {
    pub(crate) const fn change(&self) -> &ModelRouteDatabaseChange {
        &self.change
    }

    pub(crate) fn into_configuration(self) -> ModelRouteConfiguration {
        self.model_routes
    }
}

pub(crate) fn prepare_model_route_mutation(
    current: &ModelRouteConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    mutation: ModelRouteMutation,
) -> Result<Option<PreparedModelRouteMutation>, StorageError> {
    match mutation {
        ModelRouteMutation::Create { id, draft } => create(current, endpoints, id, draft).map(Some),
        ModelRouteMutation::Update {
            id,
            expected_config_version,
            draft,
        } => update(current, endpoints, id, expected_config_version, draft),
        ModelRouteMutation::Delete {
            id,
            expected_config_version,
        } => delete(current, endpoints, id, expected_config_version).map(Some),
    }
}

fn create(
    current: &ModelRouteConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    id: ModelRouteId,
    draft: ModelRouteDraft,
) -> Result<PreparedModelRouteMutation, StorageError> {
    let route = ModelRoute::create(id, draft);
    let mut routes = current.routes().to_vec();
    routes.push(route.clone());
    let configuration = ModelRouteConfiguration::new(routes, endpoints).map_err(map_validation)?;
    Ok(PreparedModelRouteMutation {
        model_routes: configuration,
        change: ModelRouteDatabaseChange::Create(route),
    })
}

fn update(
    current: &ModelRouteConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    id: ModelRouteId,
    expected_config_version: u64,
    draft: ModelRouteDraft,
) -> Result<Option<PreparedModelRouteMutation>, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::ModelRouteNotFound(id))?;
    require_version(existing.config_version(), expected_config_version)?;
    let updated = existing.updated(draft).map_err(map_validation)?;
    if &updated == existing {
        return Ok(None);
    }
    let routes = current
        .routes()
        .iter()
        .map(|route| {
            if route.id() == id {
                updated.clone()
            } else {
                route.clone()
            }
        })
        .collect();
    let configuration = ModelRouteConfiguration::new(routes, endpoints).map_err(map_validation)?;
    Ok(Some(PreparedModelRouteMutation {
        model_routes: configuration,
        change: ModelRouteDatabaseChange::Update(updated),
    }))
}

fn delete(
    current: &ModelRouteConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    id: ModelRouteId,
    expected_config_version: u64,
) -> Result<PreparedModelRouteMutation, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::ModelRouteNotFound(id))?;
    require_version(existing.config_version(), expected_config_version)?;
    let routes = current
        .routes()
        .iter()
        .filter(|route| route.id() != id)
        .cloned()
        .collect();
    let configuration = ModelRouteConfiguration::new(routes, endpoints).map_err(map_validation)?;
    Ok(PreparedModelRouteMutation {
        model_routes: configuration,
        change: ModelRouteDatabaseChange::Delete(id),
    })
}

fn require_version(actual: u64, expected: u64) -> Result<(), StorageError> {
    if actual == expected {
        Ok(())
    } else {
        Err(StorageError::ModelRouteVersionConflict { expected, actual })
    }
}

fn map_validation(error: ModelRouteValidationError) -> StorageError {
    match error {
        ModelRouteValidationError::DuplicatePublicModel => StorageError::ModelRouteNameConflict,
        ModelRouteValidationError::TargetIdentityChanged => {
            StorageError::RouteTargetIdentityConflict
        }
        ModelRouteValidationError::MissingProviderEndpoint(id) => {
            StorageError::ProviderEndpointNotFound(id)
        }
        other => StorageError::ModelRouteValidation(other),
    }
}

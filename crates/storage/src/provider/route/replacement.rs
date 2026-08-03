use any2api_domain::{ModelRoute, ModelRouteConfiguration, RouteTarget};
use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::rows::{insert_model_route, update_model_route, upsert_route_target};

pub(crate) async fn reconcile_model_routes(
    connection: &mut SqliteConnection,
    current: &ModelRouteConfiguration,
    candidate: &ModelRouteConfiguration,
) -> Result<(), StorageError> {
    for route in candidate.routes() {
        match current.get(route.id()) {
            Some(existing) => reconcile_route(connection, existing, route).await?,
            None => insert_model_route(connection, route).await?,
        }
    }
    for route in current.routes() {
        if candidate.get(route.id()).is_none() {
            delete_model_route(connection, route).await?;
        }
    }
    Ok(())
}

async fn reconcile_route(
    connection: &mut SqliteConnection,
    current: &ModelRoute,
    candidate: &ModelRoute,
) -> Result<(), StorageError> {
    if current.public_model() != candidate.public_model()
        || current.ingress_protocol() != candidate.ingress_protocol()
    {
        return Err(StorageError::CorruptConfiguration);
    }
    if route_metadata_changed(current, candidate) {
        update_model_route(connection, candidate).await?;
    }
    for target in candidate.targets() {
        match find_target(current, target) {
            Some(existing) if existing == target => {}
            _ => upsert_route_target(connection, target).await?,
        }
    }
    for target in current.targets() {
        if candidate
            .targets()
            .iter()
            .all(|candidate| candidate.id() != target.id())
        {
            delete_route_target(connection, target).await?;
        }
    }
    Ok(())
}

fn route_metadata_changed(current: &ModelRoute, candidate: &ModelRoute) -> bool {
    current.fallback_on_rate_limit() != candidate.fallback_on_rate_limit()
        || current.enabled() != candidate.enabled()
        || current.config_version() != candidate.config_version()
}

fn find_target<'a>(current: &'a ModelRoute, candidate: &RouteTarget) -> Option<&'a RouteTarget> {
    current
        .targets()
        .iter()
        .find(|target| target.id() == candidate.id())
}

async fn delete_route_target(
    connection: &mut SqliteConnection,
    target: &RouteTarget,
) -> Result<(), StorageError> {
    let result = sqlx::query("DELETE FROM route_targets WHERE id = ? AND model_route_id = ?")
        .bind(target.id().to_string())
        .bind(target.model_route_id().to_string())
        .execute(connection)
        .await?;
    require_one_row(result.rows_affected())
}

async fn delete_model_route(
    connection: &mut SqliteConnection,
    route: &ModelRoute,
) -> Result<(), StorageError> {
    let result = sqlx::query("DELETE FROM model_routes WHERE id = ?")
        .bind(route.id().to_string())
        .execute(connection)
        .await?;
    require_one_row(result.rows_affected())
}

fn require_one_row(rows_affected: u64) -> Result<(), StorageError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(StorageError::CorruptConfiguration)
    }
}

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use any2api_domain::{
    FallbackTier, ModelRoute, ModelRouteConfiguration, ModelRouteDraft, ModelRouteId,
    ProtocolDialect, ProviderEndpointConfiguration, RouteTargetDraft, RouteTargetId,
};
use sqlx::{FromRow, SqliteConnection};

use crate::{error::StorageError, model_route_mutation::ModelRouteDatabaseChange};

#[derive(FromRow)]
struct ModelRouteRow {
    id: String,
    public_model: String,
    ingress_protocol: String,
    fallback_on_saturation: Option<i64>,
    enabled: i64,
    config_version: i64,
}

#[derive(FromRow)]
struct RouteTargetRow {
    id: String,
    model_route_id: String,
    provider_endpoint_id: String,
    upstream_model: String,
    fallback_tier: i64,
    enabled: i64,
}

pub(crate) async fn load_model_routes_from(
    connection: &mut SqliteConnection,
    endpoints: &ProviderEndpointConfiguration,
) -> Result<ModelRouteConfiguration, StorageError> {
    let route_rows = sqlx::query_as::<_, ModelRouteRow>(
        "SELECT id, public_model, ingress_protocol, fallback_on_saturation, enabled, \
         config_version FROM model_routes ORDER BY ingress_protocol, public_model",
    )
    .fetch_all(&mut *connection)
    .await?;
    let target_rows = sqlx::query_as::<_, RouteTargetRow>(
        "SELECT id, model_route_id, provider_endpoint_id, upstream_model, fallback_tier, enabled \
         FROM route_targets ORDER BY model_route_id, fallback_tier, provider_endpoint_id",
    )
    .fetch_all(&mut *connection)
    .await?;
    let mut targets = group_targets(target_rows)?;
    let routes = route_rows
        .into_iter()
        .map(|row| {
            let id = row_id(&row)?;
            let route_targets = targets.remove(&id).unwrap_or_default();
            parse_route(row, route_targets)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !targets.is_empty() {
        return Err(StorageError::CorruptConfiguration);
    }
    ModelRouteConfiguration::new(routes, endpoints).map_err(|_| StorageError::CorruptConfiguration)
}

pub(crate) async fn execute_model_route_change(
    connection: &mut SqliteConnection,
    change: &ModelRouteDatabaseChange,
) -> Result<(), StorageError> {
    match change {
        ModelRouteDatabaseChange::Create(route) => insert_route(connection, route).await?,
        ModelRouteDatabaseChange::Update(route) => update_route(connection, route).await?,
        ModelRouteDatabaseChange::Delete(id) => delete_route(connection, *id).await?,
    }
    Ok(())
}

fn group_targets(
    rows: Vec<RouteTargetRow>,
) -> Result<HashMap<ModelRouteId, Vec<RouteTargetDraft>>, StorageError> {
    let mut grouped: HashMap<ModelRouteId, Vec<RouteTargetDraft>> = HashMap::new();
    for row in rows {
        let route_id = ModelRouteId::from_str(&row.model_route_id)
            .map_err(|_| StorageError::CorruptConfiguration)?;
        let id =
            RouteTargetId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)?;
        let endpoint_id = any2api_domain::ProviderEndpointId::from_str(&row.provider_endpoint_id)
            .map_err(|_| StorageError::CorruptConfiguration)?;
        let tier =
            u16::try_from(row.fallback_tier).map_err(|_| StorageError::CorruptConfiguration)?;
        let enabled = parse_bool(row.enabled)?;
        let draft = RouteTargetDraft::new(
            id,
            endpoint_id,
            row.upstream_model,
            FallbackTier::new(tier),
            enabled,
        )
        .map_err(|_| StorageError::CorruptConfiguration)?;
        grouped.entry(route_id).or_default().push(draft);
    }
    Ok(grouped)
}

fn row_id(row: &ModelRouteRow) -> Result<ModelRouteId, StorageError> {
    ModelRouteId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)
}

fn parse_route(
    row: ModelRouteRow,
    targets: Vec<RouteTargetDraft>,
) -> Result<ModelRoute, StorageError> {
    let id = ModelRouteId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)?;
    let draft = ModelRouteDraft::new(
        row.public_model,
        parse_protocol(&row.ingress_protocol)?,
        parse_optional_bool(row.fallback_on_saturation)?,
        parse_bool(row.enabled)?,
        targets,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    let version =
        u64::try_from(row.config_version).map_err(|_| StorageError::CorruptConfiguration)?;
    ModelRoute::restore(id, draft, version).map_err(|_| StorageError::CorruptConfiguration)
}

async fn insert_route(
    connection: &mut SqliteConnection,
    route: &ModelRoute,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO model_routes \
         (id, public_model, ingress_protocol, fallback_on_saturation, enabled, config_version) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(route.id().to_string())
    .bind(route.public_model().as_str())
    .bind(protocol_text(route.ingress_protocol()))
    .bind(route.fallback_on_saturation())
    .bind(route.enabled())
    .bind(i64::try_from(route.config_version()).map_err(|_| StorageError::RevisionOverflow)?)
    .execute(&mut *connection)
    .await?;
    for target in route.targets() {
        upsert_target(connection, target).await?;
    }
    Ok(())
}

async fn update_route(
    connection: &mut SqliteConnection,
    route: &ModelRoute,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE model_routes SET public_model = ?, fallback_on_saturation = ?, enabled = ?, \
         config_version = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(route.public_model().as_str())
    .bind(route.fallback_on_saturation())
    .bind(route.enabled())
    .bind(i64::try_from(route.config_version()).map_err(|_| StorageError::RevisionOverflow)?)
    .bind(route.id().to_string())
    .execute(&mut *connection)
    .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::ModelRouteNotFound(route.id()));
    }
    sync_targets(connection, route).await
}

async fn sync_targets(
    connection: &mut SqliteConnection,
    route: &ModelRoute,
) -> Result<(), StorageError> {
    let existing =
        sqlx::query_scalar::<_, String>("SELECT id FROM route_targets WHERE model_route_id = ?")
            .bind(route.id().to_string())
            .fetch_all(&mut *connection)
            .await?;
    let retained = route
        .targets()
        .iter()
        .map(|target| target.id().to_string())
        .collect::<HashSet<_>>();
    for id in existing.into_iter().filter(|id| !retained.contains(id)) {
        sqlx::query("DELETE FROM route_targets WHERE id = ?")
            .bind(id)
            .execute(&mut *connection)
            .await?;
    }
    for target in route.targets() {
        upsert_target(connection, target).await?;
    }
    Ok(())
}

async fn upsert_target(
    connection: &mut SqliteConnection,
    target: &any2api_domain::RouteTarget,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "INSERT INTO route_targets \
         (id, model_route_id, provider_endpoint_id, upstream_model, fallback_tier, enabled) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET fallback_tier = excluded.fallback_tier, \
         enabled = excluded.enabled, updated_at = CURRENT_TIMESTAMP \
         WHERE route_targets.model_route_id = excluded.model_route_id \
         AND route_targets.provider_endpoint_id = excluded.provider_endpoint_id \
         AND route_targets.upstream_model = excluded.upstream_model",
    )
    .bind(target.id().to_string())
    .bind(target.model_route_id().to_string())
    .bind(target.provider_endpoint_id().to_string())
    .bind(target.upstream_model().as_str())
    .bind(i64::from(target.fallback_tier().get()))
    .bind(target.enabled())
    .execute(&mut *connection)
    .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::CorruptConfiguration);
    }
    Ok(())
}

async fn delete_route(
    connection: &mut SqliteConnection,
    id: ModelRouteId,
) -> Result<(), StorageError> {
    let result = sqlx::query("DELETE FROM model_routes WHERE id = ?")
        .bind(id.to_string())
        .execute(connection)
        .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::ModelRouteNotFound(id));
    }
    Ok(())
}

fn parse_protocol(value: &str) -> Result<ProtocolDialect, StorageError> {
    match value {
        "openai_responses" => Ok(ProtocolDialect::OpenAiResponses),
        "anthropic_messages" => Ok(ProtocolDialect::AnthropicMessages),
        _ => Err(StorageError::CorruptConfiguration),
    }
}

const fn protocol_text(value: ProtocolDialect) -> &'static str {
    match value {
        ProtocolDialect::OpenAiResponses => "openai_responses",
        ProtocolDialect::AnthropicMessages => "anthropic_messages",
        ProtocolDialect::CodexBackend => "codex_backend",
    }
}

fn parse_bool(value: i64) -> Result<bool, StorageError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StorageError::CorruptConfiguration),
    }
}

fn parse_optional_bool(value: Option<i64>) -> Result<Option<bool>, StorageError> {
    value.map(parse_bool).transpose()
}

use std::{collections::HashMap, str::FromStr};

use any2api_domain::{
    FallbackTier, ModelRoute, ModelRouteConfiguration, ModelRouteDraft, ModelRouteId,
    ProtocolDialect, ProviderEndpointConfiguration, RouteTargetDraft, RouteTargetId,
};
use sqlx::{FromRow, SqliteConnection};

use crate::error::StorageError;

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
    upstream_protocol_dialect: String,
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
        "SELECT id, model_route_id, provider_endpoint_id, upstream_model, \
         upstream_protocol_dialect, fallback_tier, enabled \
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
            parse_protocol(&row.upstream_protocol_dialect)?,
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

pub(crate) async fn insert_model_route(
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

async fn upsert_target(
    connection: &mut SqliteConnection,
    target: &any2api_domain::RouteTarget,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "INSERT INTO route_targets \
         (id, model_route_id, provider_endpoint_id, upstream_model, \
          upstream_protocol_dialect, fallback_tier, enabled) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET fallback_tier = excluded.fallback_tier, \
         enabled = excluded.enabled, updated_at = CURRENT_TIMESTAMP \
         WHERE route_targets.model_route_id = excluded.model_route_id \
         AND route_targets.provider_endpoint_id = excluded.provider_endpoint_id \
         AND route_targets.upstream_model = excluded.upstream_model \
         AND route_targets.upstream_protocol_dialect = excluded.upstream_protocol_dialect",
    )
    .bind(target.id().to_string())
    .bind(target.model_route_id().to_string())
    .bind(target.provider_endpoint_id().to_string())
    .bind(target.upstream_model().as_str())
    .bind(protocol_text(target.upstream_protocol_dialect()))
    .bind(i64::from(target.fallback_tier().get()))
    .bind(target.enabled())
    .execute(&mut *connection)
    .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::CorruptConfiguration);
    }
    Ok(())
}

fn parse_protocol(value: &str) -> Result<ProtocolDialect, StorageError> {
    ProtocolDialect::parse(value).ok_or(StorageError::CorruptConfiguration)
}

const fn protocol_text(value: ProtocolDialect) -> &'static str {
    value.as_str()
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

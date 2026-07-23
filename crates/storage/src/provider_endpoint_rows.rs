use std::str::FromStr;

use any2api_domain::{
    ProtocolDialect, ProviderEndpoint, ProviderEndpointConfiguration, ProviderEndpointDraft,
    ProviderEndpointId, ProviderKind,
};
use sqlx::{FromRow, SqliteConnection};

use crate::{error::StorageError, provider_endpoint_mutation::ProviderEndpointDatabaseChange};

#[derive(Debug, FromRow)]
struct ProviderEndpointRow {
    id: String,
    name: String,
    provider_kind: String,
    base_url: String,
    protocol_dialect: String,
    upstream_protocol_dialect: Option<String>,
    enabled: i64,
    config_version: i64,
}

pub(crate) async fn load_provider_endpoints_from(
    connection: &mut SqliteConnection,
) -> Result<ProviderEndpointConfiguration, StorageError> {
    let rows = sqlx::query_as::<_, ProviderEndpointRow>(
        "SELECT id, name, provider_kind, base_url, protocol_dialect, upstream_protocol_dialect, \
         enabled, config_version \
         FROM provider_endpoints ORDER BY provider_kind ASC, name ASC",
    )
    .fetch_all(connection)
    .await?;
    let endpoints = rows
        .into_iter()
        .map(parse_endpoint)
        .collect::<Result<Vec<_>, _>>()?;
    ProviderEndpointConfiguration::new(endpoints).map_err(|_| StorageError::CorruptConfiguration)
}

pub(crate) async fn execute_provider_endpoint_change(
    connection: &mut SqliteConnection,
    change: &ProviderEndpointDatabaseChange,
) -> Result<(), StorageError> {
    match change {
        ProviderEndpointDatabaseChange::Create(endpoint) => insert(connection, endpoint).await?,
        ProviderEndpointDatabaseChange::Update(endpoint) => update(connection, endpoint).await?,
        ProviderEndpointDatabaseChange::Delete(id) => delete(connection, *id).await?,
    }
    Ok(())
}

async fn insert(
    connection: &mut SqliteConnection,
    endpoint: &ProviderEndpoint,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(endpoint.id().to_string())
    .bind(endpoint.name())
    .bind(endpoint.name_key())
    .bind(provider_kind_text(endpoint.provider_kind()))
    .bind(endpoint.base_url().as_str())
    .bind(protocol_dialect_text(endpoint.protocol_dialect()))
    .bind(
        endpoint
            .upstream_protocol_dialect()
            .map(protocol_dialect_text),
    )
    .bind(endpoint.enabled())
    .bind(i64::try_from(endpoint.config_version()).map_err(|_| StorageError::RevisionOverflow)?)
    .execute(connection)
    .await?;
    Ok(())
}

async fn update(
    connection: &mut SqliteConnection,
    endpoint: &ProviderEndpoint,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE provider_endpoints SET name = ?, name_key = ?, provider_kind = ?, base_url = ?, \
         protocol_dialect = ?, upstream_protocol_dialect = ?, enabled = ?, config_version = ?, \
         updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?",
    )
    .bind(endpoint.name())
    .bind(endpoint.name_key())
    .bind(provider_kind_text(endpoint.provider_kind()))
    .bind(endpoint.base_url().as_str())
    .bind(protocol_dialect_text(endpoint.protocol_dialect()))
    .bind(
        endpoint
            .upstream_protocol_dialect()
            .map(protocol_dialect_text),
    )
    .bind(endpoint.enabled())
    .bind(i64::try_from(endpoint.config_version()).map_err(|_| StorageError::RevisionOverflow)?)
    .bind(endpoint.id().to_string())
    .execute(connection)
    .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::ProviderEndpointNotFound(endpoint.id()));
    }
    Ok(())
}

async fn delete(
    connection: &mut SqliteConnection,
    id: ProviderEndpointId,
) -> Result<(), StorageError> {
    let result = sqlx::query("DELETE FROM provider_endpoints WHERE id = ?")
        .bind(id.to_string())
        .execute(connection)
        .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::ProviderEndpointNotFound(id));
    }
    Ok(())
}

fn parse_endpoint(row: ProviderEndpointRow) -> Result<ProviderEndpoint, StorageError> {
    let id =
        ProviderEndpointId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)?;
    let provider_kind = parse_provider_kind(&row.provider_kind)?;
    let protocol_dialect = parse_protocol_dialect(&row.protocol_dialect)?;
    let upstream_protocol_dialect = row
        .upstream_protocol_dialect
        .as_deref()
        .map(parse_protocol_dialect)
        .transpose()?;
    let version =
        u64::try_from(row.config_version).map_err(|_| StorageError::CorruptConfiguration)?;
    let draft = ProviderEndpointDraft::with_upstream_protocol(
        row.name,
        provider_kind,
        row.base_url,
        protocol_dialect,
        upstream_protocol_dialect,
        row.enabled == 1,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    ProviderEndpoint::restore(id, draft, version).map_err(|_| StorageError::CorruptConfiguration)
}

fn parse_provider_kind(value: &str) -> Result<ProviderKind, StorageError> {
    match value {
        "codex" => Ok(ProviderKind::Codex),
        "claude" => Ok(ProviderKind::Claude),
        _ => Err(StorageError::CorruptConfiguration),
    }
}

fn parse_protocol_dialect(value: &str) -> Result<ProtocolDialect, StorageError> {
    ProtocolDialect::parse(value).ok_or(StorageError::CorruptConfiguration)
}

const fn provider_kind_text(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Codex => "codex",
        ProviderKind::Claude => "claude",
    }
}

const fn protocol_dialect_text(dialect: ProtocolDialect) -> &'static str {
    dialect.as_str()
}

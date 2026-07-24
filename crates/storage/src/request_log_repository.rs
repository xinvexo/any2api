use std::str::FromStr;

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, ErrorClass, ProtocolDialect, ProtocolOperation,
    RequestAttempt, RequestAttemptOutcome, RequestId, RequestLog, RetrySafety,
};
use async_trait::async_trait;
use sqlx::{FromRow, SqliteConnection};

use crate::{error::StorageError, sqlite::SqliteStore};

#[async_trait]
pub trait RequestLogRepository: Send + Sync {
    async fn append_request_logs(
        &self,
        records: &[CompletedRequestLog],
    ) -> Result<(), StorageError>;

    async fn prune_request_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<u64, StorageError>;

    async fn list_request_logs(&self, limit: u32) -> Result<Vec<RequestLog>, StorageError>;

    async fn get_request_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError>;
}

#[async_trait]
impl RequestLogRepository for SqliteStore {
    async fn append_request_logs(
        &self,
        records: &[CompletedRequestLog],
    ) -> Result<(), StorageError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        for record in records {
            insert_request_log(&mut transaction, &record.request).await?;
            for attempt in &record.attempts {
                insert_request_attempt(&mut transaction, attempt).await?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn prune_request_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<u64, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let expired =
            delete_oldest_before(&mut transaction, retention_before_ms, u64::from(batch_size))
                .await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request_logs")
            .fetch_one(&mut *transaction)
            .await?;
        let count = u64::try_from(count).map_err(|_| StorageError::CorruptTelemetry)?;
        let overflow = count.saturating_sub(max_rows).min(u64::from(batch_size));
        let trimmed = if overflow == 0 {
            0
        } else {
            delete_oldest(&mut transaction, overflow).await?
        };
        transaction.commit().await?;
        Ok(expired.saturating_add(trimmed))
    }

    async fn list_request_logs(&self, limit: u32) -> Result<Vec<RequestLog>, StorageError> {
        let rows = sqlx::query_as::<_, RequestLogRow>(
            "SELECT request_id, started_at_ms, config_revision, gateway_api_key_id, \
             ingress_protocol, operation, public_model, provider_endpoint_id, credential_id, \
             oauth_account_id, proxy_profile_id, status_code, error_class, attempt_count, latency_ms, \
             first_token_ms, input_tokens, output_tokens, cache_read_tokens, \
             cache_write_tokens, is_stream FROM request_logs \
             ORDER BY started_at_ms DESC, request_id DESC LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(parse_request_log).collect()
    }

    async fn get_request_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let row = sqlx::query_as::<_, RequestLogRow>(
            "SELECT request_id, started_at_ms, config_revision, gateway_api_key_id, \
             ingress_protocol, operation, public_model, provider_endpoint_id, credential_id, \
             oauth_account_id, proxy_profile_id, status_code, error_class, attempt_count, latency_ms, \
             first_token_ms, input_tokens, output_tokens, cache_read_tokens, \
             cache_write_tokens, is_stream FROM request_logs WHERE request_id = ?",
        )
        .bind(request_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(row) = row else {
            transaction.commit().await?;
            return Ok(None);
        };
        let request = parse_request_log(row)?;
        let rows = sqlx::query_as::<_, RequestAttemptRow>(
            "SELECT request_id, attempt_no, route_target_id, credential_id, oauth_account_id, \
             proxy_profile_id, \
             started_at_ms, duration_ms, retry_safety, error_class, status_code, outcome \
             FROM request_attempts WHERE request_id = ? ORDER BY attempt_no ASC",
        )
        .bind(request_id.to_string())
        .fetch_all(&mut *transaction)
        .await?;
        let attempts = rows
            .into_iter()
            .map(parse_request_attempt)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await?;
        Ok(Some(CompletedRequestLog { request, attempts }))
    }
}

async fn insert_request_log(
    connection: &mut SqliteConnection,
    log: &RequestLog,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO request_logs (request_id, started_at_ms, config_revision, \
         gateway_api_key_id, ingress_protocol, operation, public_model, provider_endpoint_id, \
         credential_id, oauth_account_id, proxy_profile_id, status_code, error_class, \
         attempt_count, latency_ms, \
         first_token_ms, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, \
         is_stream) VALUES (?, ?, ?, (SELECT id FROM gateway_api_keys WHERE id = ?), ?, ?, ?, \
         (SELECT id FROM provider_endpoints WHERE id = ?), \
         (SELECT id FROM provider_credentials WHERE id = ?), \
         (SELECT id FROM oauth_accounts WHERE id = ?), \
         (SELECT id FROM proxy_profiles WHERE id = ?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(log.request_id.to_string())
    .bind(to_i64(log.started_at_ms)?)
    .bind(to_i64(log.config_revision.get())?)
    .bind(optional_id(log.gateway_api_key_id))
    .bind(log.ingress_protocol.as_str())
    .bind(log.operation.as_str())
    .bind(log.public_model.as_deref())
    .bind(optional_id(log.provider_endpoint_id))
    .bind(optional_id(log.credential_id))
    .bind(optional_id(log.oauth_account_id))
    .bind(optional_id(log.proxy_profile_id))
    .bind(i64::from(log.status_code))
    .bind(log.error_class.map(ErrorClass::as_str))
    .bind(i64::from(log.attempt_count))
    .bind(to_i64(log.latency_ms)?)
    .bind(optional_i64(log.first_token_ms)?)
    .bind(optional_i64(log.input_tokens)?)
    .bind(optional_i64(log.output_tokens)?)
    .bind(optional_i64(log.cache_read_tokens)?)
    .bind(optional_i64(log.cache_write_tokens)?)
    .bind(if log.is_stream { 1_i64 } else { 0_i64 })
    .execute(connection)
    .await?;
    Ok(())
}

async fn insert_request_attempt(
    connection: &mut SqliteConnection,
    attempt: &RequestAttempt,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, route_target_id, credential_id, \
         oauth_account_id, proxy_profile_id, started_at_ms, duration_ms, retry_safety, \
         error_class, status_code, \
         outcome) VALUES (?, ?, (SELECT id FROM route_targets WHERE id = ?), \
         (SELECT id FROM provider_credentials WHERE id = ?), \
         (SELECT id FROM oauth_accounts WHERE id = ?), \
         (SELECT id FROM proxy_profiles WHERE id = ?), ?, ?, ?, ?, ?, ?)",
    )
    .bind(attempt.request_id.to_string())
    .bind(i64::from(attempt.attempt_no))
    .bind(optional_id(attempt.route_target_id))
    .bind(optional_id(attempt.credential_id))
    .bind(optional_id(attempt.oauth_account_id))
    .bind(optional_id(attempt.proxy_profile_id))
    .bind(to_i64(attempt.started_at_ms)?)
    .bind(to_i64(attempt.duration_ms)?)
    .bind(attempt.retry_safety.map(RetrySafety::as_str))
    .bind(attempt.error_class.map(ErrorClass::as_str))
    .bind(attempt.status_code.map(i64::from))
    .bind(attempt.outcome.as_str())
    .execute(connection)
    .await?;
    Ok(())
}

async fn delete_oldest_before(
    connection: &mut SqliteConnection,
    cutoff_ms: u64,
    limit: u64,
) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM request_logs WHERE request_id IN (SELECT request_id FROM request_logs \
         WHERE started_at_ms < ? ORDER BY started_at_ms ASC, request_id ASC LIMIT ?)",
    )
    .bind(to_i64(cutoff_ms)?)
    .bind(to_i64(limit)?)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

async fn delete_oldest(connection: &mut SqliteConnection, limit: u64) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM request_logs WHERE request_id IN (SELECT request_id FROM request_logs \
         ORDER BY started_at_ms ASC, request_id ASC LIMIT ?)",
    )
    .bind(to_i64(limit)?)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

#[derive(FromRow)]
struct RequestLogRow {
    request_id: String,
    started_at_ms: i64,
    config_revision: i64,
    gateway_api_key_id: Option<String>,
    ingress_protocol: String,
    operation: String,
    public_model: Option<String>,
    provider_endpoint_id: Option<String>,
    credential_id: Option<String>,
    oauth_account_id: Option<String>,
    proxy_profile_id: Option<String>,
    status_code: i64,
    error_class: Option<String>,
    attempt_count: i64,
    latency_ms: i64,
    first_token_ms: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    is_stream: i64,
}

#[derive(FromRow)]
struct RequestAttemptRow {
    request_id: String,
    attempt_no: i64,
    route_target_id: Option<String>,
    credential_id: Option<String>,
    oauth_account_id: Option<String>,
    proxy_profile_id: Option<String>,
    started_at_ms: i64,
    duration_ms: i64,
    retry_safety: Option<String>,
    error_class: Option<String>,
    status_code: Option<i64>,
    outcome: String,
}

fn parse_request_log(row: RequestLogRow) -> Result<RequestLog, StorageError> {
    Ok(RequestLog {
        request_id: parse_id(&row.request_id)?,
        started_at_ms: from_i64(row.started_at_ms)?,
        config_revision: ConfigRevision::new(from_i64(row.config_revision)?)
            .map_err(|_| StorageError::CorruptTelemetry)?,
        gateway_api_key_id: parse_optional_id(row.gateway_api_key_id)?,
        ingress_protocol: ProtocolDialect::parse(&row.ingress_protocol)
            .ok_or(StorageError::CorruptTelemetry)?,
        operation: ProtocolOperation::parse(&row.operation)
            .ok_or(StorageError::CorruptTelemetry)?,
        public_model: row.public_model,
        provider_endpoint_id: parse_optional_id(row.provider_endpoint_id)?,
        credential_id: parse_optional_id(row.credential_id)?,
        oauth_account_id: parse_optional_id(row.oauth_account_id)?,
        proxy_profile_id: parse_optional_id(row.proxy_profile_id)?,
        status_code: u16::try_from(row.status_code).map_err(|_| StorageError::CorruptTelemetry)?,
        error_class: parse_optional_value(row.error_class.as_deref(), ErrorClass::parse)?,
        attempt_count: u32::try_from(row.attempt_count)
            .map_err(|_| StorageError::CorruptTelemetry)?,
        latency_ms: from_i64(row.latency_ms)?,
        first_token_ms: from_optional_i64(row.first_token_ms)?,
        input_tokens: from_optional_i64(row.input_tokens)?,
        output_tokens: from_optional_i64(row.output_tokens)?,
        cache_read_tokens: from_optional_i64(row.cache_read_tokens)?,
        cache_write_tokens: from_optional_i64(row.cache_write_tokens)?,
        is_stream: parse_bool(row.is_stream)?,
    })
}

fn parse_request_attempt(row: RequestAttemptRow) -> Result<RequestAttempt, StorageError> {
    Ok(RequestAttempt {
        request_id: parse_id(&row.request_id)?,
        attempt_no: u32::try_from(row.attempt_no).map_err(|_| StorageError::CorruptTelemetry)?,
        route_target_id: parse_optional_id(row.route_target_id)?,
        credential_id: parse_optional_id(row.credential_id)?,
        oauth_account_id: parse_optional_id(row.oauth_account_id)?,
        proxy_profile_id: parse_optional_id(row.proxy_profile_id)?,
        started_at_ms: from_i64(row.started_at_ms)?,
        duration_ms: from_i64(row.duration_ms)?,
        retry_safety: parse_optional_value(row.retry_safety.as_deref(), RetrySafety::parse)?,
        error_class: parse_optional_value(row.error_class.as_deref(), ErrorClass::parse)?,
        status_code: row
            .status_code
            .map(u16::try_from)
            .transpose()
            .map_err(|_| StorageError::CorruptTelemetry)?,
        outcome: RequestAttemptOutcome::parse(&row.outcome)
            .ok_or(StorageError::CorruptTelemetry)?,
    })
}

fn optional_id<T: ToString>(value: Option<T>) -> Option<String> {
    value.map(|value| value.to_string())
}

fn parse_optional_value<T>(
    value: Option<&str>,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<T>, StorageError> {
    value
        .map(|value| parse(value).ok_or(StorageError::CorruptTelemetry))
        .transpose()
}

fn parse_id<T: FromStr>(value: &str) -> Result<T, StorageError> {
    value.parse().map_err(|_| StorageError::CorruptTelemetry)
}

fn parse_optional_id<T: FromStr>(value: Option<String>) -> Result<Option<T>, StorageError> {
    value.map(|value| parse_id(&value)).transpose()
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn optional_i64(value: Option<u64>) -> Result<Option<i64>, StorageError> {
    value.map(to_i64).transpose()
}

fn from_i64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn from_optional_i64(value: Option<i64>) -> Result<Option<u64>, StorageError> {
    value.map(from_i64).transpose()
}

fn parse_bool(value: i64) -> Result<bool, StorageError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StorageError::CorruptTelemetry),
    }
}

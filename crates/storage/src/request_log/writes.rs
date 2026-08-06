use any2api_domain::{
    ErrorClass, RequestAttempt, RequestAttemptFailureScope, RequestAttemptRetryDecision,
    RequestLog, RequestRoutingMode, RetrySafety,
};
use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::rows::{validate_error_message, validate_thinking_level};

pub(super) async fn insert_request_log(
    connection: &mut SqliteConnection,
    log: &RequestLog,
) -> Result<(), StorageError> {
    let error_message = validate_error_message(log.error_message.as_deref())?;
    let thinking_level = validate_thinking_level(log.thinking_level.as_deref())?;
    sqlx::query(
        "INSERT INTO request_logs (request_id, started_at_ms, client_ip, config_revision, \
         gateway_api_key_id, ingress_protocol, operation, public_model, thinking_level, \
         provider_endpoint_id, credential_id, oauth_account_id, proxy_profile_id, status_code, \
         error_class, error_message, attempt_count, latency_ms, first_token_ms, input_tokens, \
         output_tokens, cache_read_tokens, is_stream) VALUES (?, ?, ?, ?, \
         (SELECT id FROM gateway_api_keys WHERE id = ?), ?, ?, ?, ?, \
         (SELECT id FROM provider_endpoints WHERE id = ?), \
         (SELECT id FROM provider_credentials WHERE id = ?), \
         (SELECT id FROM oauth_accounts WHERE id = ?), \
         (SELECT id FROM proxy_profiles WHERE id = ?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(log.request_id.to_string())
    .bind(to_i64(log.started_at_ms)?)
    .bind(log.client_ip.to_string())
    .bind(to_i64(log.config_revision.get())?)
    .bind(optional_id(log.gateway_api_key_id))
    .bind(log.ingress_protocol.as_str())
    .bind(log.operation.as_str())
    .bind(log.public_model.as_deref())
    .bind(thinking_level)
    .bind(optional_id(log.provider_endpoint_id))
    .bind(optional_id(log.credential_id))
    .bind(optional_id(log.oauth_account_id))
    .bind(optional_id(log.proxy_profile_id))
    .bind(i64::from(log.status_code))
    .bind(log.error_class.map(ErrorClass::as_str))
    .bind(error_message)
    .bind(i64::from(log.attempt_count))
    .bind(to_i64(log.latency_ms)?)
    .bind(optional_i64(log.first_token_ms)?)
    .bind(optional_i64(log.input_tokens)?)
    .bind(optional_i64(log.output_tokens)?)
    .bind(optional_i64(log.cache_read_tokens)?)
    .bind(if log.is_stream { 1_i64 } else { 0_i64 })
    .execute(connection)
    .await?;
    Ok(())
}

pub(super) async fn insert_request_attempt(
    connection: &mut SqliteConnection,
    attempt: &RequestAttempt,
) -> Result<(), StorageError> {
    let error_message = validate_error_message(attempt.error_message.as_deref())?;
    sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, route_target_id, credential_id, \
         oauth_account_id, proxy_profile_id, routing_mode, started_at_ms, duration_ms, \
         retry_safety, failure_scope, retry_decision, error_class, error_message, status_code, \
         outcome) VALUES (?, ?, \
         (SELECT id FROM route_targets WHERE id = ?), \
         (SELECT id FROM provider_credentials WHERE id = ?), \
         (SELECT id FROM oauth_accounts WHERE id = ?), \
         (SELECT id FROM proxy_profiles WHERE id = ?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(attempt.request_id.to_string())
    .bind(i64::from(attempt.attempt_no))
    .bind(optional_id(attempt.route_target_id))
    .bind(optional_id(attempt.credential_id))
    .bind(optional_id(attempt.oauth_account_id))
    .bind(optional_id(attempt.proxy_profile_id))
    .bind(attempt.routing_mode.map(RequestRoutingMode::as_str))
    .bind(to_i64(attempt.started_at_ms)?)
    .bind(to_i64(attempt.duration_ms)?)
    .bind(attempt.retry_safety.map(RetrySafety::as_str))
    .bind(
        attempt
            .failure_scope
            .map(RequestAttemptFailureScope::as_str),
    )
    .bind(
        attempt
            .retry_decision
            .map(RequestAttemptRetryDecision::as_str),
    )
    .bind(attempt.error_class.map(ErrorClass::as_str))
    .bind(error_message)
    .bind(attempt.status_code.map(i64::from))
    .bind(attempt.outcome.as_str())
    .execute(connection)
    .await?;
    Ok(())
}

pub(super) async fn delete_oldest_before(
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

pub(super) async fn delete_oldest(
    connection: &mut SqliteConnection,
    limit: u64,
) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM request_logs WHERE request_id IN (SELECT request_id FROM request_logs \
         INDEXED BY request_logs_started_idx \
         ORDER BY started_at_ms ASC, request_id ASC LIMIT ?)",
    )
    .bind(to_i64(limit)?)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

fn optional_id<T: ToString>(value: Option<T>) -> Option<String> {
    value.map(|value| value.to_string())
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn optional_i64(value: Option<u64>) -> Result<Option<i64>, StorageError> {
    value.map(to_i64).transpose()
}

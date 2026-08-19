use any2api_domain::{
    ErrorClass, MAX_TRANSPORT_WIRE_PROFILE_ID_CHARS, RequestAttempt, RequestAttemptFailureScope,
    RequestAttemptRetryDecision, RequestLog, RequestQuotaCost, RequestRoutingMode,
    RequestTelemetryPosition, RetrySafety,
};
use sqlx::{AssertSqlSafe, SqliteConnection};

use crate::error::StorageError;

use super::rows::{validate_error_message, validate_thinking_level};

pub(super) async fn insert_request_log(
    connection: &mut SqliteConnection,
    log: &RequestLog,
    telemetry_position: Option<RequestTelemetryPosition>,
) -> Result<(), StorageError> {
    let error_message = validate_error_message(log.error_message.as_deref())?;
    let thinking_level = validate_thinking_level(log.thinking_level.as_deref())?;
    let quota_cost = validate_quota_cost(log.quota_cost.as_ref())?;
    let (telemetry_process_id, telemetry_sequence) = telemetry_fields(telemetry_position)?;
    sqlx::query(
        "INSERT INTO request_logs (request_id, started_at_ms, client_ip, config_revision, \
         gateway_api_key_id, ingress_protocol, operation, public_model, thinking_level, \
         provider_endpoint_id, credential_id, oauth_account_id, proxy_profile_id, status_code, \
         error_class, error_message, attempt_count, latency_ms, first_token_ms, input_tokens, \
         output_tokens, cache_read_tokens, cache_creation_tokens, quota_cost_unit, quota_cost_nanos, \
         quota_cost_rate_card, quota_service_tier, requested_speed_tier, effective_speed_tier, \
         telemetry_process_id, telemetry_sequence, is_stream) VALUES (?, ?, ?, ?, \
         (SELECT id FROM gateway_api_keys WHERE id = ?), ?, ?, ?, ?, \
         (SELECT id FROM provider_endpoints WHERE id = ?), \
         (SELECT id FROM provider_credentials WHERE id = ?), \
         (SELECT id FROM oauth_accounts WHERE id = ?), \
         (SELECT id FROM proxy_profiles WHERE id = ?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, \
         ?, ?, ?, ?, ?)",
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
    .bind(optional_i64(log.cache_creation_tokens)?)
    .bind(quota_cost.map(|cost| cost.unit.as_str()))
    .bind(quota_cost.map(|cost| i64::try_from(cost.amount_nanos).expect("validated nano cost")))
    .bind(quota_cost.map(|cost| cost.rate_card.as_str()))
    .bind(quota_cost.map(|cost| cost.service_tier.as_str()))
    .bind(log.requested_speed_tier.map(|tier| tier.as_str()))
    .bind(log.effective_speed_tier.map(|tier| tier.as_str()))
    .bind(telemetry_process_id)
    .bind(telemetry_sequence)
    .bind(if log.is_stream { 1_i64 } else { 0_i64 })
    .execute(connection)
    .await?;
    Ok(())
}

fn telemetry_fields(
    position: Option<RequestTelemetryPosition>,
) -> Result<(Option<String>, Option<i64>), StorageError> {
    let Some(position) = position else {
        return Ok((None, None));
    };
    if position.sequence == 0 {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok((
        Some(position.process_id.to_string()),
        Some(to_i64(position.sequence)?),
    ))
}

fn validate_quota_cost(
    cost: Option<&RequestQuotaCost>,
) -> Result<Option<&RequestQuotaCost>, StorageError> {
    let Some(cost) = cost else {
        return Ok(None);
    };
    if cost.amount_nanos > i64::MAX as u64
        || RequestQuotaCost::new(
            cost.unit,
            cost.amount_nanos,
            cost.rate_card.clone(),
            cost.service_tier,
        )
        .is_none()
    {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok(Some(cost))
}

pub(super) async fn insert_request_attempt(
    connection: &mut SqliteConnection,
    attempt: &RequestAttempt,
) -> Result<(), StorageError> {
    let error_message = validate_error_message(attempt.error_message.as_deref())?;
    let transport_profile_id = attempt
        .transport
        .as_ref()
        .map(|transport| validate_transport_profile_id(&transport.wire_profile_id))
        .transpose()?;
    sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, route_target_id, credential_id, \
         oauth_account_id, proxy_profile_id, routing_mode, started_at_ms, duration_ms, \
         retry_safety, failure_scope, retry_decision, error_class, error_message, status_code, \
         outcome, transport_wire_profile_id, transport_wire_profile_version, \
         transport_timeout_policy_version, transport_resolver_mode, transport_proxy_kind, \
         transport_connect_timeout_ms, transport_read_timeout_ms, \
         transport_pool_idle_timeout_ms, transport_routing_generation, \
         transport_authentication_version, transport_traffic_class, first_upstream_frame_ms, \
         stream_commit_ms, first_downstream_byte_ms, stream_cancel_ms) VALUES (?, ?, \
         (SELECT id FROM route_targets WHERE id = ?), \
         (SELECT id FROM provider_credentials WHERE id = ?), \
         (SELECT id FROM oauth_accounts WHERE id = ?), \
         (SELECT id FROM proxy_profiles WHERE id = ?), \
         ?, ?, ?, ?, ?,  ?, ?, ?, ?, ?,  ?, ?, ?, ?, ?, \
         ?, ?, ?, ?, ?,  ?, ?, ?, ?, ?)",
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
    .bind(transport_profile_id)
    .bind(
        attempt
            .transport
            .as_ref()
            .map(|value| i64::from(value.wire_profile_version)),
    )
    .bind(
        attempt
            .transport
            .as_ref()
            .map(|value| i64::from(value.timeout_policy_version)),
    )
    .bind(
        attempt
            .transport
            .as_ref()
            .map(|value| value.resolver_mode.as_str()),
    )
    .bind(
        attempt
            .transport
            .as_ref()
            .map(|value| value.proxy_kind.as_str()),
    )
    .bind(optional_transport_i64(attempt, |value| {
        value.connect_timeout_ms
    })?)
    .bind(optional_transport_i64(attempt, |value| {
        value.read_timeout_ms
    })?)
    .bind(optional_transport_i64(attempt, |value| {
        value.pool_idle_timeout_ms
    })?)
    .bind(optional_transport_i64(attempt, |value| {
        value.routing_generation
    })?)
    .bind(optional_transport_i64(attempt, |value| {
        value.authentication_version
    })?)
    .bind(
        attempt
            .transport
            .as_ref()
            .map(|value| value.traffic_class.as_str()),
    )
    .bind(optional_stream_i64(attempt, |value| {
        value.first_upstream_frame_ms
    })?)
    .bind(optional_stream_i64(attempt, |value| {
        value.stream_commit_ms
    })?)
    .bind(optional_stream_i64(attempt, |value| {
        value.first_downstream_byte_ms
    })?)
    .bind(optional_stream_i64(attempt, |value| {
        value.stream_cancel_ms
    })?)
    .execute(connection)
    .await?;
    Ok(())
}

pub(super) async fn delete_oldest_before(
    connection: &mut SqliteConnection,
    cutoff_ms: u64,
    limit: u64,
) -> Result<u64, StorageError> {
    let victims = "SELECT request_id FROM request_logs \
         WHERE started_at_ms < ? ORDER BY started_at_ms ASC, request_id ASC LIMIT ?";
    let result = sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM request_logs WHERE request_id IN ({victims})"
    )))
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
    let victims = "SELECT request_id FROM request_logs \
         INDEXED BY request_logs_started_idx \
         ORDER BY started_at_ms ASC, request_id ASC LIMIT ?";
    let result = sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM request_logs WHERE request_id IN ({victims})"
    )))
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

fn optional_transport_i64(
    attempt: &RequestAttempt,
    get: impl FnOnce(&any2api_domain::RequestAttemptTransport) -> u64,
) -> Result<Option<i64>, StorageError> {
    attempt.transport.as_ref().map(get).map(to_i64).transpose()
}

fn optional_stream_i64(
    attempt: &RequestAttempt,
    get: impl FnOnce(any2api_domain::RequestAttemptStreamTiming) -> Option<u64>,
) -> Result<Option<i64>, StorageError> {
    attempt.stream_timing.and_then(get).map(to_i64).transpose()
}

fn validate_transport_profile_id(value: &str) -> Result<&str, StorageError> {
    if value.is_empty()
        || value.chars().count() > MAX_TRANSPORT_WIRE_PROFILE_ID_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok(value)
}

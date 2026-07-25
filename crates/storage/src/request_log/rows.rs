use std::str::FromStr;

use any2api_domain::{
    ConfigRevision, ErrorClass, MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS,
    MAX_REQUEST_LOG_THINKING_LEVEL_CHARS, ProtocolDialect, ProtocolOperation, RequestAttempt,
    RequestAttemptOutcome, RequestLog, RetrySafety,
};
use sqlx::FromRow;

use crate::error::StorageError;

#[derive(FromRow)]
pub(super) struct RequestLogRow {
    request_id: String,
    started_at_ms: i64,
    config_revision: i64,
    gateway_api_key_id: Option<String>,
    ingress_protocol: String,
    operation: String,
    public_model: Option<String>,
    thinking_level: Option<String>,
    provider_endpoint_id: Option<String>,
    credential_id: Option<String>,
    oauth_account_id: Option<String>,
    proxy_profile_id: Option<String>,
    status_code: i64,
    error_class: Option<String>,
    error_message: Option<String>,
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
pub(super) struct RequestAttemptRow {
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
    error_message: Option<String>,
    status_code: Option<i64>,
    outcome: String,
}

pub(super) fn parse_request_log(row: RequestLogRow) -> Result<RequestLog, StorageError> {
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
        thinking_level: parse_thinking_level(row.thinking_level)?,
        provider_endpoint_id: parse_optional_id(row.provider_endpoint_id)?,
        credential_id: parse_optional_id(row.credential_id)?,
        oauth_account_id: parse_optional_id(row.oauth_account_id)?,
        proxy_profile_id: parse_optional_id(row.proxy_profile_id)?,
        status_code: u16::try_from(row.status_code).map_err(|_| StorageError::CorruptTelemetry)?,
        error_class: parse_optional_value(row.error_class.as_deref(), ErrorClass::parse)?,
        error_message: parse_error_message(row.error_message)?,
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

pub(super) fn parse_request_attempt(
    row: RequestAttemptRow,
) -> Result<RequestAttempt, StorageError> {
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
        error_message: parse_error_message(row.error_message)?,
        status_code: row
            .status_code
            .map(u16::try_from)
            .transpose()
            .map_err(|_| StorageError::CorruptTelemetry)?,
        outcome: RequestAttemptOutcome::parse(&row.outcome)
            .ok_or(StorageError::CorruptTelemetry)?,
    })
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

fn parse_error_message(value: Option<String>) -> Result<Option<String>, StorageError> {
    validate_error_message(value.as_deref())?;
    Ok(value)
}

pub(super) fn validate_error_message(value: Option<&str>) -> Result<Option<&str>, StorageError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty()
        || value.trim() != value
        || value.chars().count() > MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok(Some(value))
}

fn parse_thinking_level(value: Option<String>) -> Result<Option<String>, StorageError> {
    validate_thinking_level(value.as_deref())?;
    Ok(value)
}

pub(super) fn validate_thinking_level(value: Option<&str>) -> Result<Option<&str>, StorageError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty()
        || value.trim() != value
        || value.chars().count() > MAX_REQUEST_LOG_THINKING_LEVEL_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok(Some(value))
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

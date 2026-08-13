use std::str::FromStr;

use any2api_domain::{
    ConfigRevision, ErrorClass, LogPagePosition, MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS,
    MAX_REQUEST_LOG_THINKING_LEVEL_CHARS, MAX_TRANSPORT_WIRE_PROFILE_ID_CHARS, ProtocolDialect,
    ProtocolOperation, ProxyKind, QuotaCostUnit, QuotaServiceTier, RequestAttempt,
    RequestAttemptFailureScope, RequestAttemptOutcome, RequestAttemptRetryDecision,
    RequestAttemptStreamTiming, RequestAttemptTransport, RequestLog, RequestQuotaCost,
    RequestRoutingMode, RequestTelemetryPosition, RequestTransportResolverMode,
    RequestTransportTrafficClass, RetrySafety,
};
use sqlx::FromRow;

use crate::error::StorageError;

#[derive(FromRow)]
pub(super) struct RequestLogRow {
    request_id: String,
    started_at_ms: i64,
    client_ip: String,
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
    cache_creation_tokens: Option<i64>,
    quota_cost_unit: Option<String>,
    quota_cost_nanos: Option<i64>,
    quota_cost_rate_card: Option<String>,
    quota_service_tier: Option<String>,
    telemetry_process_id: Option<String>,
    telemetry_sequence: Option<i64>,
    is_stream: i64,
}

impl RequestLogRow {
    pub(super) fn page_position(&self) -> Result<LogPagePosition, StorageError> {
        Ok(LogPagePosition::new(
            from_i64(self.started_at_ms)?,
            self.request_id.clone(),
        ))
    }

    pub(super) fn telemetry_position(
        &self,
    ) -> Result<Option<RequestTelemetryPosition>, StorageError> {
        match (
            self.telemetry_process_id.as_deref(),
            self.telemetry_sequence,
        ) {
            (None, None) => Ok(None),
            (Some(process_id), Some(sequence)) => Ok(Some(RequestTelemetryPosition {
                process_id: process_id
                    .parse()
                    .map_err(|_| StorageError::CorruptTelemetry)?,
                sequence: u64::try_from(sequence)
                    .ok()
                    .filter(|sequence| *sequence > 0)
                    .ok_or(StorageError::CorruptTelemetry)?,
            })),
            _ => Err(StorageError::CorruptTelemetry),
        }
    }
}

#[derive(FromRow)]
pub(super) struct RequestAttemptRow {
    request_id: String,
    attempt_no: i64,
    route_target_id: Option<String>,
    credential_id: Option<String>,
    oauth_account_id: Option<String>,
    proxy_profile_id: Option<String>,
    routing_mode: Option<String>,
    started_at_ms: i64,
    duration_ms: i64,
    retry_safety: Option<String>,
    failure_scope: Option<String>,
    retry_decision: Option<String>,
    error_class: Option<String>,
    error_message: Option<String>,
    status_code: Option<i64>,
    outcome: String,
    transport_wire_profile_id: Option<String>,
    transport_wire_profile_version: Option<i64>,
    transport_timeout_policy_version: Option<i64>,
    transport_resolver_mode: Option<String>,
    transport_proxy_kind: Option<String>,
    transport_connect_timeout_ms: Option<i64>,
    transport_read_timeout_ms: Option<i64>,
    transport_pool_idle_timeout_ms: Option<i64>,
    transport_routing_generation: Option<i64>,
    transport_authentication_version: Option<i64>,
    transport_traffic_class: Option<String>,
    first_upstream_frame_ms: Option<i64>,
    stream_commit_ms: Option<i64>,
    first_downstream_byte_ms: Option<i64>,
    stream_cancel_ms: Option<i64>,
}

pub(super) fn parse_request_log(row: RequestLogRow) -> Result<RequestLog, StorageError> {
    Ok(RequestLog {
        request_id: parse_id(&row.request_id)?,
        started_at_ms: from_i64(row.started_at_ms)?,
        client_ip: row
            .client_ip
            .parse()
            .map_err(|_| StorageError::CorruptTelemetry)?,
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
        cache_creation_tokens: from_optional_i64(row.cache_creation_tokens)?,
        quota_cost: parse_quota_cost(
            row.quota_cost_unit,
            row.quota_cost_nanos,
            row.quota_cost_rate_card,
            row.quota_service_tier,
        )?,
        is_stream: parse_bool(row.is_stream)?,
    })
}

fn parse_quota_cost(
    unit: Option<String>,
    amount_nanos: Option<i64>,
    rate_card: Option<String>,
    service_tier: Option<String>,
) -> Result<Option<RequestQuotaCost>, StorageError> {
    match (unit, amount_nanos, rate_card, service_tier) {
        (None, None, None, None) => Ok(None),
        (Some(unit), Some(amount_nanos), Some(rate_card), Some(service_tier)) => {
            let unit = QuotaCostUnit::parse(&unit).ok_or(StorageError::CorruptTelemetry)?;
            let service_tier =
                QuotaServiceTier::parse(&service_tier).ok_or(StorageError::CorruptTelemetry)?;
            let amount_nanos =
                u64::try_from(amount_nanos).map_err(|_| StorageError::CorruptTelemetry)?;
            RequestQuotaCost::new(unit, amount_nanos, rate_card, service_tier)
                .map(Some)
                .ok_or(StorageError::CorruptTelemetry)
        }
        _ => Err(StorageError::CorruptTelemetry),
    }
}

pub(super) fn parse_request_attempt(
    row: RequestAttemptRow,
) -> Result<RequestAttempt, StorageError> {
    let transport = parse_transport(&row)?;
    let stream_timing = parse_stream_timing(&row)?;
    Ok(RequestAttempt {
        request_id: parse_id(&row.request_id)?,
        attempt_no: u32::try_from(row.attempt_no).map_err(|_| StorageError::CorruptTelemetry)?,
        route_target_id: parse_optional_id(row.route_target_id)?,
        credential_id: parse_optional_id(row.credential_id)?,
        oauth_account_id: parse_optional_id(row.oauth_account_id)?,
        proxy_profile_id: parse_optional_id(row.proxy_profile_id)?,
        routing_mode: parse_optional_value(row.routing_mode.as_deref(), RequestRoutingMode::parse)?,
        started_at_ms: from_i64(row.started_at_ms)?,
        duration_ms: from_i64(row.duration_ms)?,
        retry_safety: parse_optional_value(row.retry_safety.as_deref(), RetrySafety::parse)?,
        failure_scope: parse_optional_value(
            row.failure_scope.as_deref(),
            RequestAttemptFailureScope::parse,
        )?,
        retry_decision: parse_optional_value(
            row.retry_decision.as_deref(),
            RequestAttemptRetryDecision::parse,
        )?,
        error_class: parse_optional_value(row.error_class.as_deref(), ErrorClass::parse)?,
        error_message: parse_error_message(row.error_message)?,
        status_code: row
            .status_code
            .map(u16::try_from)
            .transpose()
            .map_err(|_| StorageError::CorruptTelemetry)?,
        outcome: RequestAttemptOutcome::parse(&row.outcome)
            .ok_or(StorageError::CorruptTelemetry)?,
        transport,
        stream_timing,
    })
}

fn parse_transport(
    row: &RequestAttemptRow,
) -> Result<Option<RequestAttemptTransport>, StorageError> {
    let fields_present = [
        row.transport_wire_profile_id.is_some(),
        row.transport_wire_profile_version.is_some(),
        row.transport_timeout_policy_version.is_some(),
        row.transport_resolver_mode.is_some(),
        row.transport_proxy_kind.is_some(),
        row.transport_connect_timeout_ms.is_some(),
        row.transport_read_timeout_ms.is_some(),
        row.transport_pool_idle_timeout_ms.is_some(),
        row.transport_routing_generation.is_some(),
        row.transport_authentication_version.is_some(),
        row.transport_traffic_class.is_some(),
    ];
    if fields_present.iter().all(|present| !present) {
        return Ok(None);
    }
    if !fields_present.iter().all(|present| *present) {
        return Err(StorageError::CorruptTelemetry);
    }
    let wire_profile_id = row
        .transport_wire_profile_id
        .as_deref()
        .filter(|value| {
            !value.is_empty()
                && value.chars().count() <= MAX_TRANSPORT_WIRE_PROFILE_ID_CHARS
                && !value.chars().any(char::is_control)
        })
        .ok_or(StorageError::CorruptTelemetry)?
        .to_owned();
    Ok(Some(RequestAttemptTransport {
        wire_profile_id,
        wire_profile_version: parse_u16(row.transport_wire_profile_version)?,
        timeout_policy_version: parse_u16(row.transport_timeout_policy_version)?,
        resolver_mode: parse_required_value(
            row.transport_resolver_mode.as_deref(),
            RequestTransportResolverMode::parse,
        )?,
        proxy_kind: parse_required_value(row.transport_proxy_kind.as_deref(), ProxyKind::parse)?,
        connect_timeout_ms: from_required_i64(row.transport_connect_timeout_ms)?,
        read_timeout_ms: from_required_i64(row.transport_read_timeout_ms)?,
        pool_idle_timeout_ms: from_required_i64(row.transport_pool_idle_timeout_ms)?,
        routing_generation: positive_i64(row.transport_routing_generation)?,
        authentication_version: positive_i64(row.transport_authentication_version)?,
        traffic_class: parse_required_value(
            row.transport_traffic_class.as_deref(),
            RequestTransportTrafficClass::parse,
        )?,
    }))
}

fn parse_stream_timing(
    row: &RequestAttemptRow,
) -> Result<Option<RequestAttemptStreamTiming>, StorageError> {
    let timing = RequestAttemptStreamTiming {
        first_upstream_frame_ms: from_optional_i64(row.first_upstream_frame_ms)?,
        stream_commit_ms: from_optional_i64(row.stream_commit_ms)?,
        first_downstream_byte_ms: from_optional_i64(row.first_downstream_byte_ms)?,
        stream_cancel_ms: from_optional_i64(row.stream_cancel_ms)?,
    };
    Ok((!timing.is_empty()).then_some(timing))
}

fn parse_required_value<T>(
    value: Option<&str>,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<T, StorageError> {
    value.and_then(parse).ok_or(StorageError::CorruptTelemetry)
}

fn parse_u16(value: Option<i64>) -> Result<u16, StorageError> {
    value
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or(StorageError::CorruptTelemetry)
}

fn from_required_i64(value: Option<i64>) -> Result<u64, StorageError> {
    value
        .map(from_i64)
        .transpose()?
        .ok_or(StorageError::CorruptTelemetry)
}

fn positive_i64(value: Option<i64>) -> Result<u64, StorageError> {
    from_required_i64(value).and_then(|value| {
        (value > 0)
            .then_some(value)
            .ok_or(StorageError::CorruptTelemetry)
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

use std::{io::Write, time::Duration};

use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode};
use any2api_payload_buffer::PayloadBuffer;
use any2api_protocol::api::{
    DecodedRequest, ProtocolContinuationState, ProtocolError, ProtocolExchange, ProtocolRegistry,
};
use any2api_provider::api::{
    ProviderDriver, ProviderError, ProviderRegistry, ProviderRequestContext,
};
use any2api_transport::api::{EndpointNetworkPolicy, TransportProxy, TransportRequest};
use bytes::Bytes;
use http::{HeaderValue, header};

use super::PreparedAttempt;
use crate::{
    configuration::PublishedSnapshot,
    public_request::{
        SelectedCandidate, execution_limits,
        response::{internal_error, invalid_request, public_error},
        upstream::failure::AttemptFailure,
    },
    request_telemetry::{AttemptRecorder, public_error_class},
};

struct BuiltRequest<'a> {
    driver: &'a dyn ProviderDriver,
    proxy: TransportProxy<'a>,
    ingress_operation: ProtocolOperation,
    upstream_operation: ProtocolOperation,
    exchange: ProtocolExchange,
    request: TransportRequest,
}

pub(super) struct AttemptHeaderPolicy {
    pub(super) allow_credential_bound: bool,
    pub(super) allow_turn_state: bool,
}

pub(super) struct AttemptPreparation<'a, 'p, 'd> {
    pub(super) snapshot: &'a PublishedSnapshot,
    pub(super) protocols: &'p ProtocolRegistry,
    pub(super) decoded: &'d DecodedRequest,
    pub(super) selected: SelectedCandidate,
    pub(super) continuation_state: Option<ProtocolContinuationState>,
    pub(super) providers: &'a ProviderRegistry,
    pub(super) attempt_recorder: AttemptRecorder,
    pub(super) header_policy: AttemptHeaderPolicy,
}

pub(super) fn prepare_attempt<'a, 'p, 'd>(
    input: AttemptPreparation<'a, 'p, 'd>,
) -> Result<PreparedAttempt<'a>, AttemptFailure> {
    let AttemptPreparation {
        snapshot,
        protocols,
        decoded,
        selected,
        continuation_state,
        providers,
        mut attempt_recorder,
        header_policy,
    } = input;
    let result = build_request(
        snapshot,
        protocols,
        decoded,
        &selected,
        continuation_state,
        providers,
        header_policy,
    );
    let BuiltRequest {
        driver,
        proxy,
        ingress_operation,
        upstream_operation,
        exchange,
        request,
    } = match result {
        Ok(prepared) => prepared,
        Err(error) => {
            let SelectedCandidate { permit, health, .. } = selected;
            drop(health);
            attempt_recorder.local_error_before_send(
                None,
                public_error_class(error.code()),
                error.telemetry_message(),
            );
            drop(permit);
            return Err(AttemptFailure::Public(error));
        }
    };
    let SelectedCandidate { permit, health, .. } = selected;
    Ok(PreparedAttempt {
        driver,
        proxy,
        ingress_operation,
        upstream_operation,
        exchange: Some(exchange),
        request: Some(request),
        permit: Some(permit),
        health: Some(health),
        attempt_recorder: Some(attempt_recorder),
        quota_activity: None,
        oauth_account_id: None,
        quota_activity_guard: None,
    })
}

fn build_request<'a>(
    snapshot: &'a PublishedSnapshot,
    protocols: &ProtocolRegistry,
    decoded: &DecodedRequest,
    selected: &SelectedCandidate,
    continuation_state: Option<ProtocolContinuationState>,
    providers: &'a ProviderRegistry,
    header_policy: AttemptHeaderPolicy,
) -> Result<BuiltRequest<'a>, PublicError> {
    let candidate = &selected.candidate;
    let driver = providers
        .get(candidate.provider_kind)
        .ok_or_else(internal_error)?
        .as_ref();
    let proxy = snapshot
        .transport_proxy(candidate.proxy_id)
        .filter(|proxy| proxy.profile().enabled())
        .ok_or_else(|| {
            public_error(
                PublicErrorCode::NoAvailableCredential,
                "configured proxy is unavailable",
            )
        })?;
    let ingress_operation = decoded.operation;
    let execution_profile = decoded.execution_profile;
    let ingress_dialect = decoded.dialect;
    let body_encoding = decoded.body_encoding;
    let mut exchange = protocols
        .exchange(
            decoded.dialect,
            candidate.upstream_protocol_dialect,
            decoded.operation,
        )
        .map_err(|_| internal_error())?;
    let prepared = exchange
        .prepare_request(decoded, &candidate.upstream_model, continuation_state)
        .map_err(protocol_request_error)?;
    let upstream_operation = prepared.upstream_operation;
    let endpoint_plan = driver
        .endpoint_plan(&candidate.base_url, upstream_operation)
        .map_err(|_| internal_error())?;
    let mut encoded = prepared.request;
    encoded.uri = endpoint_plan
        .url
        .as_str()
        .parse()
        .map_err(|_| internal_error())?;
    let request_context = ProviderRequestContext {
        ingress_dialect,
        upstream_operation,
        upstream_model: &candidate.upstream_model,
        client_headers: &decoded.client_headers,
        oauth: selected.permit.credential_id().oauth_account_id().is_some(),
        allow_credential_bound: header_policy.allow_credential_bound,
        allow_turn_state: header_policy.allow_turn_state,
    };
    encoded.body = driver
        .prepare_request_body(request_context, encoded.body)
        .map_err(provider_request_error)?;
    let mut headers = driver
        .prepare_request_headers(request_context)
        .map_err(|_| internal_error())?;
    headers.extend(encoded.headers);
    if driver.supports_request_body_encoding(request_context, body_encoding) {
        encoded.body = encode_zstd(encoded.body)?;
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("zstd"));
    } else {
        headers.remove(header::CONTENT_ENCODING);
    }
    let credential_headers = selected
        .permit
        .credential_headers(driver, &candidate.base_url, &headers)
        .map_err(|_| internal_error())?;
    headers.extend(credential_headers.headers);
    Ok(BuiltRequest {
        driver,
        proxy,
        ingress_operation,
        upstream_operation,
        exchange,
        request: TransportRequest {
            method: encoded.method,
            uri: encoded.uri,
            headers,
            body: encoded.body,
            network_policy: EndpointNetworkPolicy::new()
                .with_strict_ssrf(snapshot.settings().upstream().strict_ssrf()),
            read_timeout: execution_limits::read_timeout(
                ingress_operation,
                execution_profile,
                Duration::from_secs(snapshot.settings().upstream().read_timeout_secs()),
            ),
        },
    })
}

fn encode_zstd(body: Bytes) -> Result<Bytes, PublicError> {
    let max_len = zstd::zstd_safe::compress_bound(body.len());
    let output = PayloadBuffer::with_capacity_hint(Some(body.len()), max_len)
        .map_err(|_| internal_error())?;
    let mut encoder = zstd::stream::write::Encoder::new(output, 3).map_err(|_| internal_error())?;
    encoder
        .write_all(body.as_ref())
        .map_err(|_| internal_error())?;
    let output = encoder.finish().map_err(|_| internal_error())?;
    Ok(output.freeze().into_bytes())
}

fn provider_request_error(error: ProviderError) -> PublicError {
    match error {
        ProviderError::Internal(_) => internal_error(),
        _ => invalid_request("request cannot be represented by the selected upstream provider"),
    }
}

fn protocol_request_error(error: ProtocolError) -> PublicError {
    match error {
        ProtocolError::SessionBindingLost => public_error(
            PublicErrorCode::SessionBindingLost,
            "previous response bridge history is unavailable",
        ),
        ProtocolError::InvalidPayload(_) => {
            invalid_request("request cannot be represented by the configured upstream protocol")
        }
        ProtocolError::ContinuationTooLarge { .. } => {
            invalid_request("request continuation exceeds the configured protocol state limit")
        }
        ProtocolError::Internal(_) => internal_error(),
        ProtocolError::DuplicateDialect(_)
        | ProtocolError::DuplicateBridge(_, _)
        | ProtocolError::Unsupported(_) => internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::encode_zstd;

    #[test]
    fn zstd_encoding_streams_large_output_into_the_payload_buffer() {
        let mut state = 0x9e37_79b9_u32;
        let input = (0..512 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect::<Vec<_>>();

        let encoded = encode_zstd(Bytes::copy_from_slice(&input)).expect("zstd encode");
        assert!(encoded.len() >= 256 * 1024);
        let decoded = zstd::stream::decode_all(encoded.as_ref()).expect("zstd decode");
        assert_eq!(decoded, input);
    }
}

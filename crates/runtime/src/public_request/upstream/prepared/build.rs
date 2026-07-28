use std::time::Duration;

use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode};
use any2api_protocol::{
    ProtocolError,
    api::{DecodedRequest, ProtocolExchange, ProtocolRegistry},
};
use any2api_provider::api::{ProviderDriver, ProviderRegistry, ProviderRequestHeaderContext};
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

pub(super) fn prepare_attempt<'a>(
    snapshot: &'a PublishedSnapshot,
    protocols: &ProtocolRegistry,
    decoded: DecodedRequest,
    selected: SelectedCandidate,
    providers: &'a ProviderRegistry,
    mut attempt_recorder: AttemptRecorder,
    header_policy: AttemptHeaderPolicy,
) -> Result<PreparedAttempt<'a>, AttemptFailure> {
    let result = build_request(
        snapshot,
        protocols,
        decoded,
        &selected,
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
                public_error_class(error.code),
                &error.message,
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
    })
}

fn build_request<'a>(
    snapshot: &'a PublishedSnapshot,
    protocols: &ProtocolRegistry,
    decoded: DecodedRequest,
    selected: &SelectedCandidate,
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
    let ingress_dialect = decoded.dialect;
    let client_headers = decoded.client_headers.clone();
    let body_encoding = decoded.body_encoding;
    let mut exchange = protocols
        .exchange(
            decoded.dialect,
            candidate.upstream_protocol_dialect,
            decoded.operation,
        )
        .map_err(|_| internal_error())?;
    let prepared = exchange
        .prepare_request(decoded, &candidate.upstream_model)
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
    let header_context = ProviderRequestHeaderContext {
        ingress_dialect,
        upstream_operation,
        upstream_model: &candidate.upstream_model,
        client_headers: &client_headers,
        oauth: selected.permit.credential_id().oauth_account_id().is_some(),
        allow_credential_bound: header_policy.allow_credential_bound,
        allow_turn_state: header_policy.allow_turn_state,
    };
    let mut headers = driver
        .prepare_request_headers(header_context)
        .map_err(|_| internal_error())?;
    headers.extend(encoded.headers);
    if driver.supports_request_body_encoding(header_context, body_encoding) {
        encoded.body = encode_zstd(encoded.body)?;
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("zstd"));
    } else {
        headers.remove(header::CONTENT_ENCODING);
    }
    let credential_headers = selected
        .permit
        .credential_headers(driver, &headers)
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
                Duration::from_secs(snapshot.settings().upstream().read_timeout_secs()),
            ),
        },
    })
}

fn encode_zstd(body: Bytes) -> Result<Bytes, PublicError> {
    zstd::stream::encode_all(body.as_ref(), 3)
        .map(Bytes::from)
        .map_err(|_| internal_error())
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
        ProtocolError::DuplicateDialect(_)
        | ProtocolError::DuplicateBridge(_, _)
        | ProtocolError::Unsupported(_) => internal_error(),
    }
}

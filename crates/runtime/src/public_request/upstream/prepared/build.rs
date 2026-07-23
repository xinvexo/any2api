use std::time::Duration;

use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode};
use any2api_protocol::{
    ProtocolError,
    api::{DecodedRequest, ProtocolExchange, ProtocolRegistry},
};
use any2api_provider::api::{ProviderDriver, ProviderRegistry};
use any2api_transport::api::{EndpointNetworkPolicy, TransportProxy, TransportRequest};

use super::PreparedAttempt;
use crate::{
    public_request::{
        SelectedCandidate,
        response::{internal_error, invalid_request, public_error},
        upstream::failure::AttemptFailure,
    },
    published_snapshot::PublishedSnapshot,
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

pub(super) fn prepare_attempt<'a>(
    snapshot: &'a PublishedSnapshot,
    protocols: &ProtocolRegistry,
    decoded: DecodedRequest,
    selected: SelectedCandidate,
    providers: &'a ProviderRegistry,
    mut attempt_recorder: AttemptRecorder,
) -> Result<PreparedAttempt<'a>, AttemptFailure> {
    let result = build_request(snapshot, protocols, decoded, &selected, providers);
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
            attempt_recorder.local_error_before_send(None, public_error_class(error.code));
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
) -> Result<BuiltRequest<'a>, PublicError> {
    let candidate = &selected.candidate;
    let endpoint = snapshot
        .provider_endpoints()
        .get(candidate.endpoint_id)
        .ok_or_else(internal_error)?;
    let driver = providers
        .get(endpoint.provider_kind())
        .ok_or_else(internal_error)?
        .as_ref();
    let proxy = snapshot
        .resolved_transport_proxy_for_credential(candidate.credential_id)
        .filter(|proxy| proxy.profile().enabled())
        .ok_or_else(|| {
            public_error(
                PublicErrorCode::NoAvailableCredential,
                "configured proxy is unavailable",
            )
        })?;
    let ingress_operation = decoded.operation;
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
        .endpoint_plan(endpoint.base_url(), upstream_operation)
        .map_err(|_| internal_error())?;
    let mut encoded = prepared.request;
    encoded.uri = endpoint_plan
        .url
        .as_str()
        .parse()
        .map_err(|_| internal_error())?;
    let credential_headers = selected
        .permit
        .provider_credential_headers(driver)
        .map_err(|_| internal_error())?;
    encoded.headers.extend(credential_headers.headers);
    Ok(BuiltRequest {
        driver,
        proxy,
        ingress_operation,
        upstream_operation,
        exchange,
        request: TransportRequest {
            method: encoded.method,
            uri: encoded.uri,
            headers: encoded.headers,
            body: encoded.body,
            network_policy: EndpointNetworkPolicy::new()
                .with_strict_ssrf(snapshot.settings().upstream().strict_ssrf()),
            read_timeout: Duration::from_secs(snapshot.settings().upstream().read_timeout_secs()),
        },
    })
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

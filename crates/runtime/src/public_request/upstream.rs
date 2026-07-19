use std::sync::Arc;

use any2api_domain::{ProtocolOperation, ProxyProfile, PublicError, PublicErrorCode};
use any2api_protocol::api::{DecodedRequest, EgressResponse, ProtocolAdapter, UpstreamResponse};
use any2api_provider::api::{ProviderDriver, ProviderRegistry, UpstreamResponseMeta};
use any2api_transport::api::{EndpointNetworkPolicy, TransportManager, TransportRequest};
use http::{HeaderValue, header};

use super::{
    PublicResponse, PublicResponseBody, RequestPermit, SelectedCandidate,
    response::{
        MAX_CLASSIFIED_ERROR_BYTES, classified_error, collect_body, internal_error,
        invalid_request, public_error, restore_public_model, sanitize_response_headers,
    },
    stream::GuardedBody,
};
use crate::published_snapshot::PublishedSnapshot;

pub(super) async fn execute_attempt(
    snapshot: &PublishedSnapshot,
    adapter: &dyn ProtocolAdapter,
    decoded: DecodedRequest,
    public_model: &str,
    selected: SelectedCandidate,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
) -> Result<EgressResponse, PublicError> {
    let prepared = prepare_attempt(snapshot, adapter, decoded, selected, providers)?;
    let transport_response = transport
        .execute(prepared.proxy, prepared.request)
        .await
        .map_err(|_| public_error(PublicErrorCode::UpstreamError, "upstream request failed"))?;
    let status = transport_response.status;
    let headers = transport_response.headers;
    let body = collect_body(transport_response.body).await?;

    if !status.is_success() {
        let classified = prepared.driver.classify_error(
            prepared.operation,
            &UpstreamResponseMeta {
                status,
                headers: headers.clone(),
            },
            &body[..body.len().min(MAX_CLASSIFIED_ERROR_BYTES)],
        );
        drop(prepared.permit);
        return Err(classified_error(classified));
    }
    drop(prepared.permit);

    let decoded = adapter
        .decode_upstream_response(UpstreamResponse {
            status,
            headers,
            body,
        })
        .map_err(|_| {
            public_error(
                PublicErrorCode::UpstreamError,
                "upstream response was invalid",
            )
        })?;
    let mut response = adapter.encode_egress_response(decoded).map_err(|_| {
        public_error(
            PublicErrorCode::UpstreamError,
            "upstream response could not be encoded",
        )
    })?;
    restore_public_model(&mut response.body, public_model)?;
    sanitize_response_headers(&mut response.headers);
    Ok(response)
}

pub(super) async fn execute_stream_attempt(
    snapshot: &PublishedSnapshot,
    adapter: Arc<dyn ProtocolAdapter>,
    decoded: DecodedRequest,
    public_model: &str,
    selected: SelectedCandidate,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
) -> Result<PublicResponse, PublicError> {
    let prepared = prepare_attempt(snapshot, adapter.as_ref(), decoded, selected, providers)?;
    let transport_response = transport
        .execute(prepared.proxy, prepared.request)
        .await
        .map_err(|_| public_error(PublicErrorCode::UpstreamError, "upstream request failed"))?;
    let status = transport_response.status;
    let mut headers = transport_response.headers;
    if !status.is_success() {
        let body = collect_body(transport_response.body).await?;
        let classified = prepared.driver.classify_error(
            prepared.operation,
            &UpstreamResponseMeta {
                status,
                headers: headers.clone(),
            },
            &body[..body.len().min(MAX_CLASSIFIED_ERROR_BYTES)],
        );
        drop(prepared.permit);
        return Err(classified_error(classified));
    }

    sanitize_response_headers(&mut headers);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    let body = GuardedBody::new(
        transport_response.body,
        adapter,
        public_model,
        prepared.permit,
    )
    .prime()
    .await?;
    Ok(PublicResponse {
        status,
        headers,
        body: PublicResponseBody::Streaming(body),
    })
}

struct PreparedAttempt<'a> {
    driver: &'a dyn ProviderDriver,
    proxy: &'a ProxyProfile,
    operation: ProtocolOperation,
    request: TransportRequest,
    permit: RequestPermit,
}

fn prepare_attempt<'a>(
    snapshot: &'a PublishedSnapshot,
    adapter: &dyn ProtocolAdapter,
    decoded: DecodedRequest,
    selected: SelectedCandidate,
    providers: &'a ProviderRegistry,
) -> Result<PreparedAttempt<'a>, PublicError> {
    let _selected_target_id = selected.candidate.target_id;
    let endpoint = snapshot
        .provider_endpoints()
        .get(selected.candidate.endpoint_id)
        .ok_or_else(internal_error)?;
    let driver = providers
        .get(endpoint.provider_kind())
        .ok_or_else(internal_error)?
        .as_ref();
    let proxy = snapshot
        .resolved_proxy_for_credential(selected.candidate.credential_id)
        .filter(|proxy| proxy.enabled())
        .ok_or_else(|| {
            public_error(
                PublicErrorCode::NoAvailableCredential,
                "configured proxy is unavailable",
            )
        })?;
    let endpoint_plan = driver
        .endpoint_plan(endpoint.base_url(), decoded.operation)
        .map_err(|_| internal_error())?;
    let operation = decoded.operation;
    let mut encoded = adapter
        .encode_upstream_request(
            decoded.operation,
            decoded.headers,
            decoded.payload,
            &selected.candidate.upstream_model,
        )
        .map_err(|_| invalid_request("request body could not be encoded"))?;
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
    Ok(PreparedAttempt {
        driver,
        proxy,
        operation,
        request: TransportRequest {
            method: encoded.method,
            uri: encoded.uri,
            headers: encoded.headers,
            body: encoded.body,
            network_policy: EndpointNetworkPolicy::new(endpoint.allow_private_network()),
        },
        permit: selected.permit,
    })
}

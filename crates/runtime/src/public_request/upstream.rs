use any2api_domain::{PublicError, PublicErrorCode};
use any2api_protocol::api::{DecodedRequest, EgressResponse, ProtocolAdapter, UpstreamResponse};
use any2api_provider::api::{ProviderRegistry, UpstreamResponseMeta};
use any2api_transport::api::{EndpointNetworkPolicy, TransportManager, TransportRequest};

use super::{
    SelectedCandidate,
    response::{
        MAX_CLASSIFIED_ERROR_BYTES, classified_error, collect_body, internal_error,
        invalid_request, public_error, restore_public_model, sanitize_response_headers,
    },
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
    let _selected_target_id = selected.candidate.target_id;
    let endpoint = snapshot
        .provider_endpoints()
        .get(selected.candidate.endpoint_id)
        .ok_or_else(internal_error)?;
    let driver = providers
        .get(endpoint.provider_kind())
        .ok_or_else(internal_error)?;
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
        .provider_credential_headers(driver.as_ref())
        .map_err(|_| internal_error())?;
    encoded.headers.extend(credential_headers.headers);

    let transport_response = transport
        .execute(
            proxy,
            TransportRequest {
                method: encoded.method,
                uri: encoded.uri,
                headers: encoded.headers,
                body: encoded.body,
                network_policy: EndpointNetworkPolicy::new(endpoint.allow_private_network()),
            },
        )
        .await
        .map_err(|_| public_error(PublicErrorCode::UpstreamError, "upstream request failed"))?;
    let status = transport_response.status;
    let headers = transport_response.headers;
    let body = collect_body(transport_response.body).await?;

    if !status.is_success() {
        let classified = driver.classify_error(
            &UpstreamResponseMeta {
                status,
                headers: headers.clone(),
            },
            &body[..body.len().min(MAX_CLASSIFIED_ERROR_BYTES)],
        );
        drop(selected.permit);
        return Err(classified_error(classified));
    }
    drop(selected.permit);

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

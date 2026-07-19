use any2api_domain::PublicErrorCode;
use any2api_protocol::api::{DecodedRequest, EgressResponse, ProtocolAdapter, UpstreamResponse};
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::TransportManager;

use super::super::{
    affinity::{AffinitySelection, commit_soft_binding},
    response::{
        collect_body, internal_error, public_error, restore_public_model, sanitize_response_headers,
    },
};
use super::{
    failure::AttemptFailure,
    prepared::{AttemptInput, hard_committer, prepare_input},
};
use crate::published_snapshot::PublishedSnapshot;

pub(in crate::public_request) async fn execute_buffered_attempt(
    snapshot: &PublishedSnapshot,
    adapter: &dyn ProtocolAdapter,
    decoded: DecodedRequest,
    public_model: &str,
    affinity: AffinitySelection,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
) -> Result<EgressResponse, AttemptFailure> {
    let AttemptInput {
        mut prepared,
        candidate,
        target,
        soft_lease,
        fixed,
    } = prepare_input(snapshot, adapter, decoded, affinity, providers)?;
    let response = match prepared.send(transport).await {
        Ok(response) => response,
        Err(error) => {
            prepared.transport_failure(&error);
            return Err(AttemptFailure::transport(error, candidate, fixed));
        }
    };
    let status = response.status;
    let headers = response.headers;
    let body = match collect_body(response.body).await {
        Ok(body) => body,
        Err(error) => {
            prepared.invalid_response();
            return Err(AttemptFailure::Public(error));
        }
    };
    if !status.is_success() {
        let classification = prepared.classify(status, &headers, &body);
        prepared.upstream_failure(classification);
        return Err(AttemptFailure::upstream(classification, candidate, fixed));
    }
    let decoded = match adapter.decode_upstream_response(UpstreamResponse {
        status,
        headers,
        body,
    }) {
        Ok(decoded) => decoded,
        Err(_) => {
            prepared.invalid_response();
            return Err(AttemptFailure::Public(public_error(
                PublicErrorCode::UpstreamError,
                "upstream response was invalid",
            )));
        }
    };
    let hard_id = adapter
        .hard_affinity_id_from_response(prepared.operation, &decoded)
        .map_err(|_| {
            prepared.fail_after_upstream_success(public_error(
                PublicErrorCode::UpstreamError,
                "upstream response identity was invalid",
            ))
        })?;
    let mut response = adapter.encode_egress_response(decoded).map_err(|_| {
        prepared.fail_after_upstream_success(public_error(
            PublicErrorCode::UpstreamError,
            "upstream response could not be encoded",
        ))
    })?;
    restore_public_model(&mut response.body, public_model)
        .map_err(|error| prepared.fail_after_upstream_success(error))?;
    sanitize_response_headers(&mut response.headers);
    let hard_affinity = hard_committer(snapshot, prepared.operation, target.clone());
    if let Some(hard_id) = hard_id {
        hard_affinity
            .bind(&hard_id)
            .map_err(|_| prepared.fail_after_upstream_success(internal_error()))?;
    }
    commit_soft_binding(soft_lease, target)
        .map_err(|error| prepared.fail_after_upstream_success(error))?;
    prepared.success();
    Ok(response)
}

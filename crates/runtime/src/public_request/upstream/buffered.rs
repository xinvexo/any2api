use std::time::Duration;

use any2api_domain::{ProtocolOperation, PublicErrorCode};
use any2api_protocol::api::{DecodedRequest, EgressResponse, ProtocolAdapter, UpstreamResponse};

use super::super::{
    affinity::{AffinitySelection, commit_soft_binding},
    response::{
        CollectBodyError, collect_body, internal_error, public_error, restore_public_model,
        sanitize_response_headers,
    },
};
use super::{
    UpstreamServices,
    failure::AttemptFailure,
    prepared::{AttemptInput, hard_committer, prepare_input},
};
use crate::request_telemetry::AttemptRecorder;

pub(in crate::public_request) async fn execute_buffered_attempt(
    services: UpstreamServices<'_>,
    adapter: &dyn ProtocolAdapter,
    decoded: DecodedRequest,
    public_model: &str,
    affinity: AffinitySelection,
    attempt_recorder: AttemptRecorder,
) -> Result<EgressResponse, AttemptFailure> {
    let AttemptInput {
        mut prepared,
        candidate,
        target,
        soft_lease,
        fixed,
    } = prepare_input(
        services.snapshot,
        adapter,
        decoded,
        affinity,
        services.providers,
        attempt_recorder,
    )?;
    let response = match prepared.send(services.transport).await {
        Ok(response) => response,
        Err(error) => {
            prepared.transport_failure(&error);
            return Err(AttemptFailure::transport(error, candidate, fixed));
        }
    };
    let status = response.status;
    let headers = response.headers;
    let body = match collect_body(
        response.body,
        Duration::from_millis(services.snapshot.settings().upstream().read_timeout_ms()),
        response.read_failure_scope,
    )
    .await
    {
        Ok(body) => body,
        Err(CollectBodyError::Transport(error)) => {
            prepared.transport_failure(&error);
            return Err(AttemptFailure::transport(error, candidate, fixed));
        }
        Err(CollectBodyError::Public(error)) => {
            prepared.invalid_response(Some(status.as_u16()));
            return Err(AttemptFailure::Public(error));
        }
    };
    if !status.is_success() {
        let classification = prepared.classify(status, &headers, &body);
        prepared.upstream_failure(status.as_u16(), classification);
        return Err(AttemptFailure::upstream(classification, candidate, fixed));
    }
    let decoded = match adapter.decode_upstream_response(UpstreamResponse {
        status,
        headers,
        body,
    }) {
        Ok(decoded) => decoded,
        Err(_) => {
            prepared.invalid_response(Some(status.as_u16()));
            return Err(AttemptFailure::Public(public_error(
                PublicErrorCode::UpstreamError,
                "upstream response was invalid",
            )));
        }
    };
    if prepared.operation != ProtocolOperation::MessagesCountTokens {
        prepared.observe_token_usage(decoded.telemetry.token_usage);
    }
    let hard_id = adapter
        .hard_affinity_id_from_response(prepared.operation, &decoded)
        .map_err(|_| {
            prepared.fail_after_upstream_success(
                status.as_u16(),
                public_error(
                    PublicErrorCode::UpstreamError,
                    "upstream response identity was invalid",
                ),
            )
        })?;
    let mut response = adapter
        .encode_egress_response(decoded)
        .map_err(|_| prepared.fail_after_upstream_success(status.as_u16(), internal_error()))?;
    restore_public_model(&mut response.body, public_model)
        .map_err(|error| prepared.fail_after_upstream_success(status.as_u16(), error))?;
    sanitize_response_headers(&mut response.headers);
    let hard_affinity = hard_committer(services.snapshot, prepared.operation, target.clone());
    if let Some(hard_id) = hard_id {
        hard_affinity
            .bind(&hard_id)
            .map_err(|_| prepared.fail_after_upstream_success(status.as_u16(), internal_error()))?;
    }
    commit_soft_binding(soft_lease, target)
        .map_err(|error| prepared.fail_after_upstream_success(status.as_u16(), error))?;
    prepared.success(status.as_u16());
    Ok(response)
}

use std::time::Duration;

use any2api_domain::{ProtocolOperation, PublicErrorCode};
use any2api_protocol::api::{
    BridgeContinuationState, DecodedRequest, EgressResponse, UpstreamResponse,
};
use http::{HeaderValue, header};

use super::super::{
    affinity::{AffinitySelection, affinity_error, commit_binding},
    execution_limits,
    response::{
        CollectBodyError, collect_body, collect_error_body, internal_error, public_error,
        sanitize_response_headers,
    },
};
use super::{
    UpstreamServices,
    failure::{AttemptFailure, UpstreamFailureOrigin},
    prepared::{AttemptInput, continuation_committer, prepare_input},
};
use crate::request_telemetry::AttemptRecorder;

pub(in crate::public_request) async fn execute_buffered_attempt(
    services: UpstreamServices<'_>,
    decoded: &DecodedRequest,
    public_model: &str,
    affinity: AffinitySelection,
    attempt_recorder: AttemptRecorder,
    allow_credential_bound_headers: bool,
) -> Result<EgressResponse, AttemptFailure> {
    let execution_profile = decoded.execution_profile;
    let AttemptInput {
        mut prepared,
        candidate,
        target,
        binding_lease,
        bound,
    } = prepare_input(
        services,
        decoded,
        affinity,
        attempt_recorder,
        allow_credential_bound_headers,
    )?;
    let response = match prepared.send(services.transport).await {
        Ok(response) => response,
        Err(error) => {
            prepared.transport_failure(&error);
            return Err(AttemptFailure::transport(error, candidate, bound));
        }
    };
    let status = response.status;
    let headers = response.headers;
    let expected_body_bytes = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    let read_timeout = execution_limits::read_timeout(
        prepared.ingress_operation,
        execution_profile,
        Duration::from_secs(
            services
                .policy_snapshot
                .settings()
                .upstream()
                .read_timeout_secs(),
        ),
    );
    if !status.is_success() {
        let collected =
            collect_error_body(response.body, read_timeout, services.attempt_deadline).await;
        let safe_headers = prepared.response_headers(&headers);
        let error = prepared.classify(status, &headers, collected.classification_bytes());
        prepared.upstream_failure(status.as_u16(), &error);
        let error_body_complete = collected.is_complete();
        let body = collected.into_response_body();
        return Err(AttemptFailure::upstream(
            UpstreamFailureOrigin::HttpStatus {
                error_body_complete,
            },
            status,
            safe_headers,
            body,
            error,
            candidate,
            bound,
        ));
    }
    let body = match collect_body(
        response.body,
        read_timeout,
        execution_limits::buffered_response_limit(prepared.ingress_operation, true),
        expected_body_bytes,
        response.read_failure_scope,
    )
    .await
    {
        Ok(body) => body,
        Err(CollectBodyError::Transport(error)) => {
            prepared.transport_failure(&error);
            return Err(AttemptFailure::transport(error, candidate, bound));
        }
        Err(CollectBodyError::Public(error)) => {
            prepared.invalid_response(Some(status.as_u16()), error.telemetry_message());
            return Err(AttemptFailure::Public(error));
        }
    };
    let upstream = UpstreamResponse {
        status,
        headers,
        body,
    };
    if let Some(evidence) = prepared.buffered_upstream_failure(&upstream) {
        let safe_headers = prepared.response_headers(&upstream.headers);
        let error = prepared.classify_evidence(status, &upstream.headers, &evidence);
        prepared.upstream_failure(status.as_u16(), &error);
        return Err(AttemptFailure::upstream(
            UpstreamFailureOrigin::BufferedEnvelope,
            status,
            safe_headers,
            upstream.body,
            error,
            candidate,
            bound,
        ));
    }
    let safe_headers = prepared.response_headers(&upstream.headers);
    let decoded = match prepared.decode_upstream_response(upstream) {
        Ok(decoded) => decoded,
        Err(_) => {
            prepared.invalid_response(Some(status.as_u16()), "upstream response was invalid");
            return Err(AttemptFailure::Public(public_error(
                PublicErrorCode::UpstreamError,
                "upstream response was invalid",
            )));
        }
    };
    if prepared.ingress_operation != ProtocolOperation::MessagesCountTokens {
        prepared.observe_token_usage(decoded.telemetry.token_usage);
    }
    let continuation_id = prepared
        .continuation_id_from_response(&decoded)
        .map_err(|_| {
            prepared.fail_after_upstream_success(
                status.as_u16(),
                public_error(
                    PublicErrorCode::UpstreamError,
                    "upstream response identity was invalid",
                ),
            )
        })?;
    let continuation_state = prepared.bridge_continuation_state();
    let mut response = prepared
        .encode_egress_response(decoded, public_model)
        .map_err(|_| prepared.fail_after_upstream_success(status.as_u16(), internal_error()))?;
    response.headers = safe_headers;
    response.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    sanitize_response_headers(&mut response.headers);
    let continuation_binding = continuation_committer(
        services.policy_snapshot,
        prepared.ingress_operation,
        target.clone(),
    );
    if let Some(continuation_id) = continuation_id {
        let state = match continuation_state {
            BridgeContinuationState::Stateless => None,
            BridgeContinuationState::Ready(state) => Some(state),
            BridgeContinuationState::Pending => {
                return Err(prepared.fail_after_upstream_success(
                    status.as_u16(),
                    public_error(
                        PublicErrorCode::UpstreamError,
                        "upstream response continuation was incomplete",
                    ),
                ));
            }
        };
        continuation_binding
            .bind_ready(&continuation_id, state)
            .map_err(|error| {
                prepared.fail_after_upstream_success(status.as_u16(), affinity_error(error))
            })?;
    } else if !matches!(continuation_state, BridgeContinuationState::Stateless) {
        return Err(prepared.fail_after_upstream_success(
            status.as_u16(),
            public_error(
                PublicErrorCode::UpstreamError,
                "upstream response continuation identity was missing",
            ),
        ));
    }
    commit_binding(binding_lease, target)
        .map_err(|error| prepared.fail_after_upstream_success(status.as_u16(), error))?;
    prepared.success(status.as_u16());
    Ok(response)
}

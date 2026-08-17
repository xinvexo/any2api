use std::time::Duration;

use any2api_protocol::api::DecodedRequest;
use http::{HeaderValue, header};

use super::super::{
    PublicResponse, PublicResponseBody,
    affinity::{AffinitySelection, commit_binding_before},
    execution_limits,
    response::{collect_error_body, sanitize_response_headers},
    stream::{GuardedBody, GuardedBodyParts, PrecommitBudget},
};
use super::{
    UpstreamServices,
    failure::{AttemptFailure, UpstreamFailureOrigin},
    prepared::{AttemptInput, PreparedStreamGuards, continuation_committer, prepare_input},
};
use crate::request_telemetry::{AttemptRecorder, public_error_class};

pub(in crate::public_request) async fn execute_stream_attempt(
    services: UpstreamServices<'_>,
    decoded: &DecodedRequest,
    public_model: String,
    affinity: AffinitySelection,
    attempt_recorder: AttemptRecorder,
    allow_credential_bound_headers: bool,
) -> Result<PublicResponse, AttemptFailure> {
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
    let upstream_headers = response.headers;
    let read_timeout = execution_limits::read_timeout(
        prepared.ingress_operation,
        execution_profile,
        Duration::from_secs(services.snapshot.settings().upstream().read_timeout_secs()),
    );
    if !status.is_success() {
        let collected =
            collect_error_body(response.body, read_timeout, services.attempt_deadline).await;
        let safe_headers = prepared.response_headers(&upstream_headers);
        let error = prepared.classify(status, &upstream_headers, collected.classification_bytes());
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
    let mut headers = prepared.response_headers(&upstream_headers);
    sanitize_response_headers(&mut headers);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    let continuation_binding = continuation_committer(
        services.snapshot,
        prepared.ingress_operation,
        target.clone(),
    );
    let precommit_budget = PrecommitBudget::from_settings(
        prepared.ingress_operation,
        execution_profile,
        services.snapshot.settings().stream(),
    );
    let PreparedStreamGuards {
        exchange,
        permit,
        health,
        attempt_recorder,
        quota_activity,
        driver,
        upstream_operation,
    } = prepared.take_guards();
    let body = GuardedBody::new(
        response.body,
        exchange,
        public_model,
        GuardedBodyParts {
            permit,
            health,
            continuation_binding,
            attempt_recorder,
            quota_activity,
            status_code: status.as_u16(),
            driver,
            upstream_operation,
            upstream_headers,
            precommit_budget,
            postcommit_idle_timeout: execution_limits::stream_timeout(
                prepared.ingress_operation,
                execution_profile,
                Duration::from_secs(
                    services
                        .snapshot
                        .settings()
                        .stream()
                        .postcommit_idle_timeout_secs(),
                ),
            ),
        },
    );
    let mut body = match body.prime_attempt().await {
        Ok(body) => body,
        Err(super::super::stream::StreamPrimeFailure::Upstream(failure)) => {
            return Err(AttemptFailure::upstream(
                UpstreamFailureOrigin::PreSemanticStreamEnvelope {
                    retry_delay_basis: failure.retry_delay_basis,
                },
                status,
                headers,
                failure.frame,
                failure.error,
                candidate,
                bound,
            ));
        }
        Err(super::super::stream::StreamPrimeFailure::Public(error)) => {
            return Err(AttemptFailure::Public(error));
        }
    };
    let precommit_deadline = body.precommit_deadline();
    if let Err(error) = commit_binding_before(binding_lease, target, precommit_deadline) {
        body.fail_before_handoff(public_error_class(error.code()), error.telemetry_message());
        return Err(AttemptFailure::Public(error));
    }
    body.commit_precommit_continuation(Some(precommit_deadline))
        .map_err(AttemptFailure::Public)?;
    Ok(PublicResponse {
        status,
        headers,
        body: PublicResponseBody::Streaming(body.into_stream()),
    })
}

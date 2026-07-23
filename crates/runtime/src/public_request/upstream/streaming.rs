use std::time::Duration;

use any2api_protocol::api::DecodedRequest;
use http::{HeaderValue, header};

use super::super::{
    PublicResponse, PublicResponseBody,
    affinity::{AffinitySelection, commit_soft_binding_before},
    response::{CollectBodyError, collect_body, sanitize_response_headers},
    stream::{GuardedBody, GuardedBodyParts, PrecommitBudget},
};
use super::{
    UpstreamServices,
    failure::AttemptFailure,
    prepared::{AttemptInput, hard_committer, prepare_input},
};
use crate::request_telemetry::{AttemptRecorder, public_error_class};

pub(in crate::public_request) async fn execute_stream_attempt(
    services: UpstreamServices<'_>,
    decoded: DecodedRequest,
    public_model: String,
    affinity: AffinitySelection,
    attempt_recorder: AttemptRecorder,
) -> Result<PublicResponse, AttemptFailure> {
    let AttemptInput {
        mut prepared,
        candidate,
        target,
        soft_lease,
        fixed,
    } = prepare_input(
        services.snapshot,
        services.protocols,
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
    let mut headers = response.headers;
    let read_failure_scope = response.read_failure_scope;
    let read_timeout =
        Duration::from_secs(services.snapshot.settings().upstream().read_timeout_secs());
    if !status.is_success() {
        let body = match collect_body(response.body, read_timeout, read_failure_scope).await {
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
        let classification = prepared.classify(status, &headers, &body);
        prepared.upstream_failure(status.as_u16(), classification);
        return Err(AttemptFailure::upstream(classification, candidate, fixed));
    }
    sanitize_response_headers(&mut headers);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    let hard_affinity = hard_committer(
        services.snapshot,
        prepared.ingress_operation,
        target.clone(),
    );
    let precommit_budget = PrecommitBudget::from_settings(services.snapshot.settings().stream());
    let (exchange, permit, health, attempt_recorder) = prepared.take_guards();
    let mut body = GuardedBody::new(
        response.body,
        exchange,
        public_model,
        GuardedBodyParts {
            permit,
            health,
            hard_affinity,
            attempt_recorder,
            status_code: status.as_u16(),
            precommit_budget,
            postcommit_idle_timeout: Duration::from_secs(
                services
                    .snapshot
                    .settings()
                    .stream()
                    .postcommit_idle_timeout_secs(),
            ),
        },
    )
    .prime()
    .await
    .map_err(AttemptFailure::Public)?;
    if let Err(error) = commit_soft_binding_before(soft_lease, target, body.precommit_deadline()) {
        body.fail_before_handoff(public_error_class(error.code));
        return Err(AttemptFailure::Public(error));
    }
    Ok(PublicResponse {
        status,
        headers,
        body: PublicResponseBody::Streaming(body.into_stream()),
    })
}

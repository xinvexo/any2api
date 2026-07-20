use std::sync::Arc;

use any2api_protocol::api::{DecodedRequest, ProtocolAdapter};
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
    adapter: Arc<dyn ProtocolAdapter>,
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
        adapter.as_ref(),
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
    if !status.is_success() {
        let body = match collect_body(response.body).await {
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
    let hard_affinity = hard_committer(services.snapshot, prepared.operation, target.clone());
    let precommit_budget = PrecommitBudget::from_settings(services.snapshot.settings().stream());
    let (permit, health, attempt_recorder) = prepared.take_guards();
    let mut body = GuardedBody::new(
        response.body,
        adapter,
        public_model,
        GuardedBodyParts {
            permit,
            health,
            hard_affinity,
            attempt_recorder,
            status_code: status.as_u16(),
            precommit_budget,
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
